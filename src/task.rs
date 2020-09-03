//! Task implementation
//!
//! The task wraps a future and allows it to be started by the executor.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::sync::atomic::{Ordering, AtomicU8};
use core::ops::{BitAnd, BitOr};
use crate::task_list::TaskList;

#[macro_export]
macro_rules! task_decl {
    ($name:ident, $val:expr) => {
        let $name = uio::task::Task::new($val,
            {
                static WAKER: uio::task::TaskWaker = uio::task::TaskWaker::new();
                &WAKER
            });
        $crate::pin_utils::pin_mut!($name);
    };
}

#[macro_export]
macro_rules! task_start {
    ($name:ident, $val:expr) => {
        $crate::task_decl!($name, $val);
        let $name = uio::executor::start($name);
    };
}

pub struct TaskWaker {
    ready_flag: AtomicU8,
}

impl TaskWaker {
    pub const fn new() -> Self {
        Self {
            ready_flag: AtomicU8::new(0)
        }
    }

    fn update_flag(&self, update_fn: impl Fn(u8) -> u8) -> u8 {
        let mut flag_value = self.ready_flag.load(Ordering::Acquire);
        let mut new_value = update_fn(flag_value);
        while self.ready_flag.compare_and_swap(flag_value, new_value, Ordering::SeqCst) != flag_value {
            flag_value = self.ready_flag.load(Ordering::Acquire);
            new_value = update_fn(flag_value);
        }

        flag_value
    }

    pub(crate) fn set_started(&self) {
        self.update_flag(|value| value.bitor(0b0000_0010));
    }

    pub(crate) fn set_finished(&self) {
        self.update_flag(|value| value.bitand(0b1111_1101));
    }

    pub(crate) fn set_ready_to_poll(&self) {
        self.update_flag(|value| value.bitor(0x01));
    }

    pub(crate) fn clear_ready_to_poll(&self) {
        self.update_flag(|value| value.bitand(0b1111_1110));
    }

    pub(crate) fn try_take_reference(&self) -> bool {
        let mut flag_value = self.ready_flag.load(Ordering::Acquire);
        if flag_value & 0b0000_0100 != 0 {
            return false;
        }
        let mut new_value = flag_value | 0b0000_0100;
        while self.ready_flag.compare_and_swap(flag_value, new_value, Ordering::SeqCst) != flag_value {
            flag_value = self.ready_flag.load(Ordering::Acquire);
            if flag_value & 0b0000_0100 != 0 {
                return false;
            }
            new_value = flag_value | 0b0000_0100;
        }
        return true;
    }

    pub(crate) fn release_reference(&self) -> bool {
        let mut flag_value = self.ready_flag.load(Ordering::Acquire);
        if flag_value & 0b0000_0100 == 0 {
            return false;
        }

        let mut new_value = flag_value & !0b0000_0100;

        while self.ready_flag.compare_and_swap(flag_value, new_value, Ordering::SeqCst) != flag_value {
            flag_value = self.ready_flag.load(Ordering::Acquire);
            if flag_value & 0b0000_0100 == 0 {
                return false;
            }
            new_value = flag_value & !0b0000_0100;
        }
        return true;
    }
}

/// Any tasks type used by the executor must store an instance of this structure. It must not be changed
/// by the task once the task has been started.
///
/// It is used internally by the executor and cannot be used in any other way.
///
/// # Panics
///
/// Panics if any instance of TaskData is dropped while the owning task is still active within the
/// executor.
pub struct TaskData {
    pub(crate) waker: &'static TaskWaker,
    pub(crate) next: *mut dyn crate::executor::Task,
}

impl TaskData {
    /// Constructs a new task-data instance. This is the only function available.
    /// Only used when constructing task-objects.
    pub fn new(waker: &'static TaskWaker) -> Self {
        if !waker.try_take_reference() {
            panic!("Attempting to reuse waker already in use by a different task.");
        }
        Self {
            waker,
            next: TaskList::end_item(),
        }
    }

    pub(crate) fn is_ready_to_poll(&self) -> bool {
        (self.waker.ready_flag.load(Ordering::Acquire) & 0x01) > 0
    }
}

/// If TaskData is in use by the executor (started but not finished, ready for poll etc.)
/// when it is dropped the drop code will panic.
impl Drop for TaskData {
    fn drop(&mut self) {
        if !self.waker.release_reference() {
            panic!("Releasing an already released reference!");
        }
        if (self.waker.ready_flag.load(Ordering::Acquire) & 0b0000_0010) != 0 {
            panic!("Task dropped while it is still running");
        }
    }
}

/// Task structure, wrapping a future allowing it to be run by the executor.
pub struct Task<T: Future> {
    future: T,
    task_data: TaskData,
    value: crate::future::Value<T::Output>,
}

impl<T: Future> Unpin for Task<T> {}

impl<T: Future> Task<T> {
    /// Create a new task wrapping the specified `future`.
    pub fn new(future: T, waker: &'static TaskWaker) -> Self {
        Self {
            future,
            task_data: crate::task::TaskData::new(waker),
            value: crate::future::Value::new(),
        }
    }
}

impl<T: Future> crate::executor::Task for Task<T> {
    fn mut_task_data(&mut self) -> &mut TaskData {
        &mut self.task_data
    }

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let future = unsafe { Pin::new_unchecked(&mut self.future) };
        match future.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(r) => {
                self.value.set(r);
                Poll::Ready(())
            }
        }
    }
}

impl<T: Future> crate::executor::TypedTask for Task<T> {
    type Output = T::Output;
    fn value_ptr(&mut self) -> *mut crate::future::Value<Self::Output> {
        &mut self.value as *mut crate::future::Value<Self::Output>
    }
}
