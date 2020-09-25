use core::future::Future;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use core::sync::atomic::{AtomicU8, Ordering};

pub trait Cancelable {
    fn cancel_future(&mut self);
}

const HAS_VALUE_FLAG: u8 = 0b0000_0001;
const HAS_WAKER_FLAG: u8 = 0b0000_0010;

pub struct Value<T> {
    value: MaybeUninit<T>,
    waker: MaybeUninit<Waker>,
    flags: AtomicU8,
}

impl<T> Value<T> {
    pub fn new() -> Self {
        Self {
            value: MaybeUninit::uninit(),
            waker: MaybeUninit::uninit(),
            flags: AtomicU8::new(0),
        }
    }

    pub fn set(&mut self, value: T) {
        self.value = MaybeUninit::new(value);
        self.flags.fetch_or(HAS_VALUE_FLAG, Ordering::AcqRel);
        self.take_waker().and_then(|w| {
            w.wake();
            Some(())
        });
    }

    fn has_value(&self) -> bool {
        (self.flags.load(Ordering::Acquire) & HAS_VALUE_FLAG) != 0
    }

    fn has_waker(&self) -> bool {
        (self.flags.load(Ordering::Acquire) & HAS_WAKER_FLAG) != 0
    }

    fn take_waker(&mut self) -> Option<Waker> {
        if !self.has_waker() {
            None
        } else {
            self.flags.fetch_and(!HAS_WAKER_FLAG, Ordering::AcqRel);
            Some(unsafe {
                core::mem::replace(&mut self.waker, MaybeUninit::uninit()).assume_init()
            })
        }
    }

    unsafe fn take_value(&mut self) -> T {
        self.flags.fetch_and(!HAS_VALUE_FLAG, Ordering::AcqRel);
        core::mem::replace(&mut self.value, MaybeUninit::uninit()).assume_init()
    }
}

impl<T> Drop for Value<T> {
    fn drop(&mut self) {
        self.take_waker();
        if self.has_value() {
            unsafe { self.take_value() }
        }
    }
}

impl<T: core::marker::Unpin> core::marker::Unpin for Value<T> {}

impl<T: core::marker::Unpin> Future for Value<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        //let mut_self = unsafe { self.get_unchecked_mut() };

        if self.has_value() {
            Poll::Ready(unsafe { self.take_value() })
        } else {
            self.waker = MaybeUninit::new(cx.waker().clone());
            self.flags.fetch_or(HAS_WAKER_FLAG, Ordering::AcqRel);
            Poll::Pending
        }
    }
}
