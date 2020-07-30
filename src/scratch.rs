
use core::future::Future;
use core::task::{Context, Poll};
use core::task::Poll::*;
use core::pin::Pin;

use std::thread::spawn;

pub struct IrqFuture {
    sleep_time: u64,
    value: Option<()>
}

impl IrqFuture{
    pub fn new() -> Self {
        Self::new_with_sleep(5)
    }

    pub fn new_with_sleep(sleep_time: u64) -> Self {
        Self {
            sleep_time,
            value: None,
        }
    }
}

impl Future for IrqFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if self.value.is_some() {
            Ready(self.value.unwrap())
        }
        else {
            let mut_self = unsafe { self.get_unchecked_mut() };
            mut_self.value = Some(());
            let sleep_time = mut_self.sleep_time;
            let waker = cx.waker().clone();
            spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(sleep_time));
                waker.wake();
            });
            Pending
        }
    }
}