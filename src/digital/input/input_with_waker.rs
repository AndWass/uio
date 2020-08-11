use embedded_hal::digital::InputPin;
use crate::digital::input::{AsyncInputPin, HighInputPin, LowInputPin};

pub struct InputWithWaker<P> {
    pin: P,
    waker: &'static crate::interrupt::Waker
}

impl<P> InputWithWaker<P> {
    pub fn new(pin: P, waker: &'static crate::interrupt::Waker) -> Self {
        Self {
            pin,
            waker
        }
    }

    pub fn free(self) -> P {
        self.pin
    }
}

impl<P> InputPin for InputWithWaker<P>
    where P: InputPin {
    type Error = P::Error;

    fn try_is_high(&self) -> Result<bool, Self::Error> {
        self.pin.try_is_high()
    }

    fn try_is_low(&self) -> Result<bool, Self::Error> {
        self.pin.try_is_low()
    }
}

impl<P> AsyncInputPin for InputWithWaker<P>
    where P: InputPin {
    fn wait_until_high<'a>(&'a self) -> HighInputPin<'a, Self> {
        HighInputPin::new(self, self.waker)
    }

    fn wait_until_low<'a>(&'a self) -> LowInputPin<'a, Self> {
        LowInputPin::new(self, self.waker)
    }
}