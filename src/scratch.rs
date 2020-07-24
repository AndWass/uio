
use core::future::Future;
use core::task::{Context, Poll};
use core::task::Poll::*;
use core::pin::Pin;

struct IrqFuture {
    value: Option<()>,
}

impl Future for IrqFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.value.is_some() {
            Ready(self.value.unwrap())
        }
        else {
            Pending
        }
    }
}