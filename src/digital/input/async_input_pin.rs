use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

pub trait AsyncInputPin {
    type Error;
    fn poll_high(&mut self, cx: &mut Context) -> Poll<Result<(), Self::Error>>;
    fn poll_low(&mut self, cx: &mut Context) -> Poll<Result<(), Self::Error>>;
}

pub trait AsyncInputExt {
    type Error;
    fn wait_until_high(&mut self) -> HighInputFuture<Self>;
    fn wait_until_low(&mut self) -> LowInputFuture<Self>;
}

impl<T> AsyncInputExt for T
where
    T: AsyncInputPin
{
    type Error = T::Error;

    fn wait_until_high(&mut self) -> HighInputFuture<Self> {
        HighInputFuture {
            pin: self,
        }
    }

    fn wait_until_low(&mut self) -> LowInputFuture<Self> {
        LowInputFuture {
            pin: self,
        }
    }
}

pub struct HighInputFuture<'a, P: ?Sized> {
    pin: &'a mut P,
}

impl<'a, P: ?Sized> core::marker::Unpin for HighInputFuture<'a, P> {}

impl<P> Future for HighInputFuture<'_, P>
where
    P: AsyncInputPin + ?Sized,
{
    type Output = Result<(), P::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.pin.poll_high(cx)
    }
}

impl<P> crate::future::Cancelable for HighInputFuture<'_, P>
where
    P: crate::future::Cancelable + ?Sized,
{
    fn cancel_future(&mut self) {
        self.pin.cancel_future();
    }
}

pub struct LowInputFuture<'a, P: ?Sized> {
    pin: &'a mut P,
}

impl<'a, P: ?Sized> core::marker::Unpin for LowInputFuture<'a, P> {}

impl<P> Future for LowInputFuture<'_, P>
where
    P: AsyncInputPin + ?Sized,
{
    type Output = Result<(), P::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.pin.poll_low(cx)
    }
}

impl<P> crate::future::Cancelable for LowInputFuture<'_, P>
    where
        P: crate::future::Cancelable + ?Sized
{
    fn cancel_future(&mut self) {
        self.pin.cancel_future()
    }
}
