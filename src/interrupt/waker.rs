use core::cell::Cell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use core::task;

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
        if self
            .has_waker
            .compare_and_swap(true, false, Ordering::SeqCst)
        {
            unsafe { Some(self.take_waker_impl()) }
        } else {
            None
        }
    }

    pub fn try_wake(&self) -> bool {
        self.take_waker()
            .and_then(|w| {
                w.wake();
                Some(true)
            })
            .unwrap_or(false)
    }

    pub fn has_waker(&self) -> bool {
        self.has_waker.load(Ordering::Acquire)
    }
}

unsafe impl Sync for Waker {}

pub struct WakerRef {
    waker: AtomicPtr<Waker>,
}

impl WakerRef {
    unsafe fn waker_ref(&self) -> Option<&Waker> {
        let ptr = self.waker.load(Ordering::Acquire) as *const Waker;
        ptr.as_ref()
    }

    pub const fn new_empty() -> Self {
        Self {
            waker: AtomicPtr::new(core::ptr::null_mut()),
        }
    }

    pub fn new(waker: &Waker) -> Self {
        Self {
            waker: AtomicPtr::new(waker as *const Waker as *mut Waker),
        }
    }

    pub fn assign(&self, other: &Self) {
        self.waker
            .store(other.waker.load(Ordering::Acquire), Ordering::Release);
    }

    pub fn try_wake(&self) -> bool {
        unsafe {
            self.waker_ref()
                .and_then(|w| Some(w.try_wake()))
                .or(Some(false)).expect("")
        }
    }

    pub fn set_waker(&self, waker: task::Waker) {
        unsafe {
            self.waker_ref().and_then(|w| Some(w.set_waker(waker)));
        }
    }

    pub fn take_waker(&self) -> Option<task::Waker> {
        unsafe { self.waker_ref().and_then(|w| w.take_waker()) }
    }
}

impl Clone for WakerRef {
    fn clone(&self) -> Self {
        Self {
            waker: AtomicPtr::new(self.waker.load(Ordering::Acquire)),
        }
    }
}

unsafe impl Send for WakerRef {}
