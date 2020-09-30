//! Task implementation
//!
//! The task wraps a future and allows it to be started by the executor.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::sync::atomic::{AtomicU8, Ordering};
use core::ops::{BitAnd, BitOr};
use embedded_async::intrusive::intrusive_list::Node;

struct PlaceholderTask;
impl crate::executor::Task for PlaceholderTask {
    fn waker(&self) -> &'static TaskWaker {
        panic!("Attempting to use the placeholder task!");
    }

    fn node(&mut self) -> &mut Node<*mut dyn crate::executor::Task> {
        panic!("Attempting to use the placeholder task!");
    }

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        panic!("Attempting to use the placeholder task!");
    }
}

static mut PLACEHOLDER_TASK: PlaceholderTask = PlaceholderTask;

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

    pub(crate) fn is_finished(&self) -> bool {
        (self.ready_flag.load(Ordering::Acquire) & 0x02) == 0
    }

    pub(crate) fn set_started(&self) {
        self.update_flag(|value| value.bitor(0b0000_0010));
    }

    pub(crate) fn set_finished(&self) {
        self.update_flag(|value| value.bitand(0b1111_1101));
    }

    pub(crate) fn is_ready_to_poll(&self) -> bool {
        (self.ready_flag.load(Ordering::Acquire) & 0x01) > 0
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

#[macro_export]
macro_rules! task_decl {
    ($name:ident, $val:expr) => {
        let $name = uio::task::Task::new($val, { static WAKER: uio::task::TaskWaker = uio::task::TaskWaker::new(); &WAKER});
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

struct TaskData {
    waker: &'static TaskWaker
}

impl TaskData {
    pub fn new(waker: &'static TaskWaker) -> Self {
        if !waker.try_take_reference() {
            panic!("Attempting to reuse waker already in use by a different task.");
        }
        Self {
            waker,
        }
    }
}

impl Drop for TaskData {
    fn drop(&mut self) {
        if !self.waker.release_reference() {
            panic!("Releasing an already released reference!");
        }
        if !self.waker.is_finished() {
            panic!("Task dropped while it is still running");
        }
    }
}

/// Task structure, wrapping a future allowing it to be run by the executor.
pub struct Task<T: Future> {
    future: T,
    task_data: TaskData,
    list_node: embedded_async::intrusive::intrusive_list::Node<*mut dyn crate::executor::Task>,
    value: crate::future::Value<T::Output>,
}

impl<T: Future> Unpin for Task<T> {}

impl<T: Future> Task<T> {
    /// Create a new task wrapping the specified `future`.
    pub fn new(future: T, waker: &'static TaskWaker) -> Self {
        Self {
            future,
            task_data: TaskData::new(waker),
            list_node: embedded_async::intrusive::intrusive_list::Node::new(unsafe { &mut PLACEHOLDER_TASK }),
            value: crate::future::Value::new(),
        }
    }
}

impl<T: Future> crate::executor::Task for Task<T> {
    fn waker(&self) -> &'static TaskWaker {
        &self.task_data.waker
    }

    fn node(&mut self) -> &mut Node<*mut dyn crate::executor::Task> {
        &mut self.list_node
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
