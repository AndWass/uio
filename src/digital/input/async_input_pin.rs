
use embedded_hal::digital::InputPin;

use core::future::Future;
use core::task::{Context, Poll};
use core::pin::Pin;

pub trait AsyncInputPin: InputPin {
    fn wait_until_high(&self) -> HighInputPin<Self>;
    fn wait_until_low(&self) -> LowInputPin<Self>;
}

pub struct HighInputPin<'a, P: ?Sized> {
    pin: &'a P,
    waker: &'a crate::interrupt::Waker,
}

impl<'a, P: ?Sized> core::marker::Unpin for HighInputPin<'a, P> {

}

impl<'a, P: ?Sized> HighInputPin<'a, P> {
    pub fn new(pin: &'a P, waker: &'a crate::interrupt::Waker) -> Self {
        Self {
            pin,
            waker
        }
    }
}

impl<P> Future for HighInputPin<'_, P>
    where P: InputPin + ?Sized {
    type Output = Result<(), <P as embedded_hal::digital::InputPin>::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.waker.set_waker(cx.waker().clone());
        match self.pin.try_is_high() {
            Err(e) => {
                self.waker.take_waker();
                Poll::Ready(Err(e))
            },
            Ok(true) => {
                self.waker.take_waker();
                Poll::Ready(Ok(()))
            },
            Ok(false) => {
                Poll::Pending
            }
        }
    }
}

pub struct LowInputPin<'a, P: ?Sized> {
    pin: &'a P,
    waker: &'a crate::interrupt::Waker,
}

impl<'a, P: ?Sized> LowInputPin<'a, P> {
    pub fn new(pin: &'a P, waker: &'a crate::interrupt::Waker) -> Self {
        Self {
            pin,
            waker
        }
    }
}

impl<'a, P: ?Sized> core::marker::Unpin for LowInputPin<'a, P> {}

impl<P> Future for LowInputPin<'_, P>
    where P: InputPin + ?Sized {
    type Output = Result<(), <P as embedded_hal::digital::InputPin>::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.waker.set_waker(cx.waker().clone());
        match self.pin.try_is_high() {
            Err(e) => {
                self.waker.take_waker();
                Poll::Ready(Err(e))
            },
            Ok(false) => {
                self.waker.take_waker();
                Poll::Ready(Ok(()))
            },
            Ok(true) => {
                Poll::Pending
            }
        }
    }
}