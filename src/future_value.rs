use core::task::{Waker, Poll, Context};
use core::future::Future;
use core::pin::Pin;

pub struct FutureValue<T> {
    value: Option<T>,
    waker: Option<Waker>,
}

impl<T> FutureValue<T> {
    pub fn new() -> Self {
        Self {
            value: None,
            waker: None,
        }
    }

    pub fn set(&mut self, value: T) {
        self.value = Some(value);
        if self.waker.is_some() {
            self.waker.take().unwrap().wake();
        }
    }
}

impl<T> Future for FutureValue<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut_self = unsafe { self.get_unchecked_mut() };

        if mut_self.value.is_some() {
            Poll::Ready(mut_self.value.take().unwrap())
        }
        else {
            mut_self.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}