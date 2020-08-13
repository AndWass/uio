use crate::interrupt::{Waker, WakerRef, Wakable};
use core::pin::Pin;
use core::task::{Context, Poll};

pub trait PollRead {
    type Error;
    fn poll_read(
        &mut self,
        waker: WakerRef,
        buffer: &mut [u8],
    ) -> Poll<Result<usize, Self::Error>>;
}

#[cfg(embedded_hal_impl = "serial_read")]
impl<E> PollRead for dyn embedded_hal::serial::Read<u8, Error = E> {
    type Error = E;
    fn poll_read(
        &mut self,
        _waker: WakerRef,
        buffer: &mut [u8],
    ) -> Poll<Result<usize, Self::Error>> {
        match self.try_read() {
            Ok(x) => {
                buffer[0] = x;
                Poll::Ready(Ok(1))
            },
            Err(nb::Error::WouldBlock) => {
                Poll::Pending
            },
            Err(nb::Error::Other(err)) => {
                Poll::Ready(Err(err))
            }
        }
    }
}

pub trait AsyncRead: PollRead + Wakable {
    fn read_some<'a>(&'a mut self, buffer: &'a mut [u8]) -> ReadFuture<'a, Self> {
        let waker = self.waker_ref();
        ReadFuture {
            reader: self,
            buffer: buffer,
            waker: waker,
        }
    }
}

impl<T> AsyncRead for T
where T: PollRead + Wakable
{
}

pub struct ReadFuture<'a, T: ?Sized> {
    pub(crate) reader: &'a mut T,
    pub(crate) buffer: &'a mut [u8],
    pub(crate) waker: WakerRef,
}

impl<'a, T> core::future::Future for ReadFuture<'a, T>
where
    T: ?Sized + PollRead,
{
    type Output = Result<usize, T::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            reader,
            buffer,
            waker,
        } = &mut *self;

        waker.set_waker(cx.waker().clone());

        match reader.poll_read(waker.clone(), buffer) {

            Poll::Pending => Poll::Pending,
            Poll::Ready(r) => {
                waker.take_waker();
                Poll::Ready(r)
            }
        }
    }
}

pub struct ReaderWithWaker<R> {
    reader: R,
    waker: &'static Waker,
}

impl<R: PollRead> ReaderWithWaker<R> {
    pub fn new(reader: R, waker: &'static Waker) -> Self {
        Self { reader, waker }
    }
}

impl<R: PollRead> PollRead for ReaderWithWaker<R>
{
    type Error = R::Error;
    fn poll_read(
        &mut self,
        waker: WakerRef,
        buffer: &mut [u8],
    ) -> Poll<Result<usize, Self::Error>> {
        self.reader.poll_read(waker, buffer)
    }
}

impl<R> Wakable for ReaderWithWaker<R>
{
    fn waker_ref(&self) -> WakerRef {
        WakerRef::new(self.waker)
    }
}

pub async fn read<R: AsyncRead>(
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
