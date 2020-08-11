use core::task;
use core::sync::atomic::{AtomicBool, Ordering};
use core::mem::MaybeUninit;
use core::cell::Cell;

pub struct Waker {
    waker: core::cell::Cell<MaybeUninit<task::Waker>>,
    has_waker: AtomicBool,
}

impl Waker {
    pub const fn new() -> Self {
        Self {
            waker: Cell::new(MaybeUninit::uninit()),
            has_waker: AtomicBool::new(false),
        }
    }
    unsafe fn take_waker_impl(&self) -> task::Waker {
        self.waker.replace(MaybeUninit::uninit()).assume_init()
    }
    pub fn set_waker(&self, waker: task::Waker) {
        self.has_waker.store(false, Ordering::Release);
        self.waker.replace(MaybeUninit::new(waker));
        self.has_waker.store(true, Ordering::Release);
    }

    pub fn take_waker(&self) -> Option<task::Waker> {
        if self.has_waker.compare_and_swap(true, false, Ordering::SeqCst) {
            unsafe {
                Some(self.take_waker_impl())
            }
        }
        else {
            None
        }
    }

    pub fn try_wake(&self) -> bool {
        self.take_waker().and_then(|w| {
            w.wake();
            Some(true)
        }).unwrap_or(false)
    }
}

unsafe impl core::marker::Sync for Waker {

}