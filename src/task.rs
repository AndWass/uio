
use core::future::Future;
use core::task::{Context, Poll};
use core::pin::Pin;
use crate::executor::TaskData;

pub struct Task<V, T: Future<Output = V>> {
    future: T,
    task_data: crate::executor::TaskData,
    result: crate::future_value::FutureValue<V>,
}

impl<V, T: Future<Output = V>> Task<V, T> {
    pub fn new(future: T) -> Self {
        Self {
            future,
            task_data: crate::executor::TaskData::new(),
            result: crate::future_value::FutureValue::new(),
        }
    }

    pub fn join_handle(&mut self) -> &mut crate::future_value::FutureValue<V> {
        &mut self.result
    }
}

impl<V, T: Future<Output = V>> crate::executor::Task for Task<V, T> {
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