//! Task implementation
//!
//! The task wraps a future and allows it to be started by the executor.

use crate::executor::TaskData;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

#[macro_export]
macro_rules! task_decl {
    ($name:ident, $val:expr) => {
        let $name = uio::task::Task::new($val);
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

/// Task structure, wrapping a future allowing it to be run by the executor.
pub struct Task<T: Future> {
    future: T,
    task_data: TaskData,
    value: crate::future::Value<T::Output>,
}

impl<T: Future> Unpin for Task<T> {}

impl<T: Future> Task<T> {
    /// Create a new task wrapping the specified `future`.
    pub fn new(future: T) -> Self {
        Self {
            future,
            task_data: crate::executor::TaskData::new(),
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
