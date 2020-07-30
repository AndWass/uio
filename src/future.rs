use core::task::{Waker, Poll, Context};
use core::future::Future;
use core::pin::Pin;
use core::mem::MaybeUninit;

use core::sync::atomic::{AtomicU8, Ordering};

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
        }
        else {
            self.flags.fetch_and(!HAS_WAKER_FLAG, Ordering::AcqRel);
            Some(unsafe { core::mem::replace(&mut self.waker, MaybeUninit::uninit()).assume_init() })
        }
    }

    unsafe fn take_value(&mut self) -> T {
        self.flags.fetch_and(!HAS_VALUE_FLAG, Ordering::AcqRel);
        core::mem::replace(&mut self.value, MaybeUninit::uninit()).assume_init()
    }
}

impl<T> Future for Value<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut_self = unsafe { self.get_unchecked_mut() };

        if mut_self.has_value() {
            Poll::Ready(unsafe { mut_self.take_value() })
        }
        else {
            mut_self.waker = MaybeUninit::new(cx.waker().clone());
            mut_self.flags.fetch_or(HAS_WAKER_FLAG, Ordering::AcqRel);
            Poll::Pending
        }
    }
}