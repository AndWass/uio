use core::pin::Pin;
use core::task::{Context, Poll};

pub trait AsyncRead {
    type Error;
    fn poll_read(&mut self, cx: &mut Context, buffer: &mut [u8])
        -> Poll<Result<usize, Self::Error>>;
}

pub trait AsyncReadExt: AsyncRead {
    fn read_some<'a>(&'a mut self, buffer: &'a mut [u8]) -> ReadFuture<'a, Self> {
        ReadFuture {
            reader: self,
            buffer,
        }
    }
}

impl<T> AsyncReadExt for T where T: AsyncRead {}

pub struct ReadFuture<'a, T: ?Sized> {
    pub(crate) reader: &'a mut T,
    pub(crate) buffer: &'a mut [u8],
}

impl<'a, T> core::future::Future for ReadFuture<'a, T>
where
    T: ?Sized + AsyncRead,
{
    type Output = Result<usize, T::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            reader,
            buffer,
        } = &mut *self;

        reader.poll_read(cx, buffer)
    }
}

impl<'a, T> crate::future::Cancelable for ReadFuture<'a, T>
where
    T: ?Sized + AsyncRead + crate::future::Cancelable
{
    fn cancel_future(&mut self) {
        self.reader.cancel_future();
    }
}

pub async fn read<R: AsyncReadExt>(
    reader: &mut R,
    buffer: &mut [u8],
) -> Result<(), (usize, R::Error)> {
    let mut amount_read = 0usize;
    loop {
        match reader.read_some(&mut buffer[amount_read..]).await {
            Ok(read_now) => {
                amount_read += read_now;
            }
            Err(e) => {
                return Err((amount_read, e));
            }
        }

        if amount_read == buffer.len() {
            return Ok(());
        } else if amount_read > buffer.len() {
            panic!("Amount read is greater than buffer length!");
        }
    }
}
