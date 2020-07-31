//! Task implementation
//!
//! The task wraps a future and allows it to be started by the executor.

use core::future::Future;
use core::task::{Context, Poll};
use core::pin::Pin;
use crate::executor::TaskData;
use crate::future::Value;

/// Task structure, wrapping a future allowing it to be run by the executor.
pub struct Task<T: Future> {
    future: T,
    task_data: TaskData,
    result: Value<T::Output>,
}

impl<T: Future> Task<T> {
    /// Create a new task wrapping the specified `future`.
    pub fn new(future: T) -> Self {
        Self {
            future,
            task_data: crate::executor::TaskData::new(),
            result: crate::future::Value::new(),
        }
    }

    /// Get a reference to a "join-handle" that can be awaited on.
    pub fn join_handle(&mut self) -> &mut Value<T::Output> {
        &mut self.result
    }
}

impl<T: Future> crate::executor::Task for Task<T> {
    fn mut_task_data(&mut self) -> &mut TaskData {
        &mut self.task_data
    }

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let mut_self = unsafe { self.get_unchecked_mut() };
        let future = unsafe { Pin::new_unchecked(&mut mut_self.future) };
        match future.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(r) => {
                mut_self.result.set(r);
                Poll::Ready(())
            }
        }
    }
}