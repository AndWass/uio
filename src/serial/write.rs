use crate::interrupt::{Waker, WakerRef, Wakable};
use core::pin::Pin;
use core::task::{Context, Poll};

pub trait PollWrite {
    type Error;
    fn poll_write(&mut self, buffer: &[u8]) -> Poll<Result<usize, Self::Error>>;
}

pub trait AsyncWrite: PollWrite + Wakable {
    fn write_some<'a>(&'a mut self, buffer: &'a [u8]) -> WriteFuture<'a, Self> {
        let waker = self.waker_ref();
        WriteFuture {
            writer: self,
            buffer: buffer,
            waker: waker,
        }
    }
}

impl<T> AsyncWrite for T
where T: PollWrite + Wakable {
    
}

pub struct WriteFuture<'a, T: ?Sized> {
    pub(crate) writer: &'a mut T,
    pub(crate) buffer: &'a [u8],
    pub(crate) waker: WakerRef,
}

impl<'a, T> core::future::Future for WriteFuture<'a, T>
where
    T: ?Sized + PollWrite,
{
    type Output = Result<usize, T::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            writer,
            buffer,
            waker,
        } = &mut *self;
        waker.set_waker(cx.waker().clone());

        match writer.poll_write(buffer) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(r) => {
                waker.take_waker();
                Poll::Ready(r)
            }
        }
    }
}

pub struct WriterWithWaker<Writer> {
    writer: Writer,
    waker: &'static Waker,
}

impl<Writer: PollWrite> WriterWithWaker<Writer> {
    pub fn new(writer: Writer, waker: &'static Waker) -> Self {
        Self { writer, waker }
    }
}

impl<Writer: PollWrite> PollWrite for WriterWithWaker<Writer> {
    type Error = Writer::Error;
    fn poll_write(&mut self, buffer: &[u8]) -> Poll<Result<usize, Self::Error>> {
        self.writer.poll_write(buffer)
    }
}

impl<T> Wakable for WriterWithWaker<T> {
    fn waker_ref(&self) -> WakerRef {
        WakerRef::new(&self.waker)
    }
}

pub async fn write<W: AsyncWrite>(writer: &mut W, buffer: &[u8]) -> Result<(), (usize, W::Error)> {
    let mut amount_written = 0usize;
    loop {
        match writer.write_some(&buffer[amount_written..]).await {
            Ok(x) => {
                amount_written += x;
            }
            Err(err) => {
                return Err((amount_written, err));
            }
        }

        if amount_written == buffer.len() {
            return Ok(());
        }
        else if amount_written > buffer.len() {
            panic!("Amount written is greater than buffer length!")
        }
    }
}

