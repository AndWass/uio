use core::task::Poll;

pub struct SendFuture<'a, T> {
    channel: &'a Channel<'a, T>,
    value: Option<T>,
}

impl<T> Unpin for SendFuture<'_, T> {}

impl<T: Clone> core::future::Future for SendFuture<'_, T> {
    type Output = ();
    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        let value = self.value.take().unwrap();
        match self.channel.try_send(value) {
            Ok(()) => {
                Poll::Ready(())
            },
            Err(msg) => {
                self.channel.data.borrow_mut().waiting_writer = Some(cx.waker().clone());
                self.value = Some(msg);
                Poll::Pending
            }
        }
    }
}

pub struct Sender<'a, T>
{
    channel: &'a Channel<'a, T>
}

impl<'a, T: Clone> Sender<'a, T> {
    pub fn capacity(&self) -> usize {
        self.channel.capacity()
    }

    pub async fn send(&self, message: T) {
        SendFuture {
            channel: self.channel,
            value: Some(message)
        }.await
    }
}

pub struct RecvFuture<'a, T> {
    channel: &'a Channel<'a, T>
}

impl<T> Unpin for RecvFuture<'_, T> {}

impl<T: Clone> core::future::Future for RecvFuture<'_, T> {
    type Output = T;
    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        match self.channel.try_recv() {
            Ok(value) => {
                Poll::Ready(value)
            },
            Err(_) => {
                self.channel.data.borrow_mut().waiting_writer = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}


pub struct Receiver<'a, T>
{
    channel: &'a Channel<'a, T>
}

impl<'a, T: Clone> Receiver<'a, T> {
    pub fn capacity(&self) -> usize {
        self.channel.capacity()
    }

    pub async fn recv(&self) -> T {
        RecvFuture {
            channel: self.channel
        }.await
    }
}

struct ChannelData<'a, T>
{
    buffer: &'a mut [T],
    read_pos: usize,
    write_pos: usize,
    waiting_reader: Option<core::task::Waker>,
    waiting_writer: Option<core::task::Waker>,
}

impl<'a, T> ChannelData<'a, T>
{
    fn notify_waiting_reader(&mut self) {
        self.waiting_reader.take().and_then(|w| Some(w.wake()));
    }

    fn notify_waiting_writer(&mut self) {
        self.waiting_writer.take().and_then(|w| Some(w.wake()));
    }
}

pub struct Channel<'a, T>
{
    data: core::cell::RefCell<ChannelData<'a, T>>,
}

impl<'a, T: Clone> Channel<'a, T> {
    pub fn new(buffer: &'a mut [T]) -> Self {
        Self {
            data: core::cell::RefCell::new(ChannelData {
                buffer,
                read_pos: 0,
                write_pos: 0,
                waiting_reader: None,
                waiting_writer: None,
            })
        }
    }

    pub fn receiver(&'a self) -> Receiver<'a, T> {
        Receiver {
            channel: &self
        }
    }

    pub fn sender(&'a self) -> Sender<'a, T> {
        Sender {
            channel: &self
        }
    }

    pub fn capacity(&self) -> usize {
        self.data.borrow().buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        let data = self.data.borrow();
        data.read_pos == data.write_pos
    }

    pub fn is_full(&self) -> bool {
        let data = self.data.borrow();
        if data.read_pos == 0 {
            // About to wrap around and read is a 0, so we are full!
            data.write_pos == data.buffer.len() - 1
        }
        else if data.read_pos > data.write_pos {
            // About to catch up to read position, so we are full
            data.write_pos + 1 == data.read_pos
        }
        else {
            false
        }
    }

    fn try_send(&self, message: T) -> Result<(), T> {
        if self.is_full() {
            Err(message)
        }
        else {
            let mut data = self.data.borrow_mut();
            let write_pos = data.write_pos;
            data.buffer[write_pos] = message;
            data.write_pos = (write_pos + 1) % data.buffer.len();
            data.notify_waiting_reader();
            Ok(())
        }
    }

    fn try_recv(&self) -> Result<T, ()> {
        if self.is_empty() {
            Err(())
        }
        else {
            let mut data = self.data.borrow_mut();
            let read_pos = data.read_pos;
            let retval = Ok(data.buffer[read_pos].clone());
            data.read_pos = (read_pos + 1) % data.buffer.len();
            data.notify_waiting_writer();
            retval
        }
    }
}