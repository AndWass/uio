use core::task::{Context, Waker, Poll};
use core::pin::Pin;
use core::time::Duration;

pub fn duration_to_ticks(duration: Duration, tick_frequency: u32) -> Result<u32, ()> {
    let seconds = duration.as_secs() as u32;

    if let Some(sec_ticks) = tick_frequency.checked_mul(seconds) {
        let mut subsec_nanos = duration.subsec_nanos();
        let mut ticks_to_add = 0u32;

        if subsec_nanos > 1000_000 {
            // add subsec milliseconds
            let millis = subsec_nanos / 1_000_000;
            ticks_to_add += (tick_frequency / 1_000) * millis;
            subsec_nanos -= millis * 1_000_000;
        }

        if subsec_nanos > 1000 {
            let micros = subsec_nanos / 1_000;
            ticks_to_add += (tick_frequency / 1_000_000) * micros;
            subsec_nanos -= micros * 1000;
        }

        if subsec_nanos > 0 && tick_frequency > 1_000_000_000 {
            ticks_to_add += (tick_frequency / 1_000_000_000) * subsec_nanos;
        }
        if ticks_to_add <= u32::max_value() {
            match sec_ticks.checked_add(ticks_to_add) {
                Some(result) => Ok(result),
                _ => Err(())
            }
        }
        else {
            Err(())
        }
    }
    else {
        Err(())
    }
}

pub struct DelayFuture<'a, T: WakeAfter + ?Sized> {
    timer: &'a mut T,
    duration: Duration,
    timer_started: bool
}

impl<T: WakeAfter + ?Sized> core::future::Future for DelayFuture<'_, T> {
    type Output = Result<(), T::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.timer_started {
            if self.timer.wake_signaled() {
                Poll::Ready(Ok(()))
            }
            else {
                Poll::Pending
            }
        }
        else {
            let duration = self.duration;
            match self.timer.wake_after(cx.waker().clone(), duration) {
                Err(e) => Poll::Ready(Err(e)),
                _ => {
                    self.timer_started = true;
                    Poll::Pending
                }
            }
        }
    }
}

pub trait WakeAfter {
    type Error;
    fn wake_after(&mut self, waker: Waker, duration: core::time::Duration)
                 -> Result<(), Self::Error>;

    fn wake_signaled(&self) -> bool;
}

pub trait DelayExt: WakeAfter {
    fn delay(&mut self, duration: Duration) -> DelayFuture<Self> {
        DelayFuture{
            timer: self,
            duration,
            timer_started: false,
        }
    }
}

impl<T: WakeAfter> DelayExt for T {}

