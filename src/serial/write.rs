use core::pin::Pin;
use core::task::{Context, Poll};

pub trait AsyncWrite {
    type Error;
    fn poll_write(&mut self, cx: &mut Context, buffer: &[u8]) -> Poll<Result<usize, Self::Error>>;
}

pub trait AsyncWriteExt: AsyncWrite {
    fn write_some<'a>(&'a mut self, buffer: &'a [u8]) -> WriteFuture<'a, Self> {
        WriteFuture {
            writer: self,
            buffer: buffer,
        }
    }
}

impl<T> AsyncWriteExt for T where T: AsyncWrite {}

pub struct WriteFuture<'a, T: ?Sized> {
    pub(crate) writer: &'a mut T,
    pub(crate) buffer: &'a [u8],
}

impl<'a, T> core::future::Future for WriteFuture<'a, T>
where
    T: ?Sized + AsyncWrite,
{
    type Output = Result<usize, T::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            writer,
            buffer,
        } = &mut *self;

        writer.poll_write(cx, buffer)
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
        } else if amount_written > buffer.len() {
            panic!("Amount written is greater than buffer length!")
        }
    }
}
