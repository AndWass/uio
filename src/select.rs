use core::pin::Pin;
use core::future::Future;
use core::task::{Context, Poll};

use crate::future::Cancelable;

pub struct FirstFuture<T, U>
where
    T: Cancelable,
    U: Cancelable
{
    fut1: T,
    fut2: U,
}

impl<T, U> Unpin for FirstFuture<T, U>
where
    T: Cancelable,
    U: Cancelable
{}

impl<T, U> Future for FirstFuture<T, U>
where
    T: Cancelable + Future,
    U: Cancelable + Future
{
    type Output = (Option<T::Output>, Option<U::Output>);

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            fut1,
            fut2
        } = &mut *self;
        let fut1 = unsafe { Pin::new_unchecked(fut1) };
        let fut2 = unsafe { Pin::new_unchecked(fut2) };
        match (fut1.poll(cx), fut2.poll(cx)) {
            (Poll::Pending, Poll::Pending) => {
                Poll::Pending
            },
            (Poll::Ready(v1), Poll::Ready(v2)) => {
                Poll::Ready((Some(v1), Some(v2)))
            },
            (Poll::Ready(v1), Poll::Pending) => {
                self.fut2.cancel_future();
                Poll::Ready((Some(v1), None))
            },
            (Poll::Pending, Poll::Ready(v2)) => {
                self.fut1.cancel_future();
                Poll::Ready((None, Some(v2)))
            }
        }
    }
}

impl<T, U> Cancelable for FirstFuture<T, U>
where
    T: Cancelable,
    U: Cancelable
{
    fn cancel_future(&mut self) {
        self.fut1.cancel_future();
        self.fut2.cancel_future();
    }
}

pub fn first<T: Cancelable, U: Cancelable>(fut1: T, fut2: U) -> FirstFuture<T, U> {
    FirstFuture {
        fut1,
        fut2,
    }
}

pub struct BothFuture<T, U>
where
    T: Future,
    U: Future
{
    fut1: T,
    fut2: U,
    fut1_value: Option<T::Output>,
    fut2_value: Option<U::Output>
}

impl<T, U> Unpin for BothFuture<T, U>
where
    T: Future,
    U: Future
{}

impl<T, U> Future for BothFuture<T, U>
where
    T: Future,
    U: Future
{
    type Output = (T::Output, U::Output);

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            fut1,
            fut2,
            fut1_value,
            fut2_value
        } = &mut *self;

        let fut1 = unsafe { Pin::new_unchecked(fut1) };
        let fut2 = unsafe { Pin::new_unchecked(fut2) };

        if fut1_value.is_none() {
            *fut1_value = match fut1.poll(cx) {
                Poll::Pending => None,
                Poll::Ready(val) => Some(val),
            };
        }

        if fut2_value.is_none() {
            *fut2_value = match fut2.poll(cx) {
                Poll::Pending => None,
                Poll::Ready(val) => Some(val),
            };
        }

        match (fut1_value.is_some(), fut2_value.is_some()) {
            (true, true) => {
                Poll::Ready((fut1_value.take().expect(""), fut2_value.take().expect("")))
            },
            _ => Poll::Pending
        }
    }
}
impl<T, U> Cancelable for BothFuture<T, U>
where
    T: Cancelable + Future,
    U: Cancelable + Future
{
    fn cancel_future(&mut self) {
        self.fut1.cancel_future();
        self.fut2.cancel_future();
    }
}

pub fn both<T: Future, U: Future>(fut1: T, fut2: U) -> BothFuture<T, U> {
    BothFuture {
        fut1,
        fut2,
        fut1_value: None,
        fut2_value: None,
    }
}