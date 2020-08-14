use core::task::{Poll, Waker};

use core::cell::UnsafeCell;
use core::pin::Pin;

pub struct SendError {}

pub struct RecvError {}

enum TrySendError<T> {
    Full(T),
    Disconnected(T),
}

enum TryRecvError {
    Empty,
    Disconnected,
}

trait ChannelTrait<T: Clone> {
    fn capacity(&self) -> usize;
    fn try_send(&self, value: T) -> Result<(), TrySendError<T>>;
    fn try_recv(&self) -> Result<T, TryRecvError>;
    fn add_waiting_sender(&self, future: &SendFuture<T>);
    fn add_waiting_receiver(&self, future: &RecvFuture<T>);

    fn add_sender(&self);
    fn drop_sender(&self);
    fn add_receiver(&self);
    fn drop_receiver(&self);
}

// SendFuture is only used in
// Sender<T>.send, will be added to
// a list of available futures if there
// are no
struct SendFuture<T: Clone> {
    sender: *const Sender<T>,
    value: Option<T>,
    waker: UnsafeCell<Option<Waker>>,
    next: UnsafeCell<*const SendFuture<T>>,
}

impl<T: Clone> SendFuture<T> {
    fn sender(&self) -> &Sender<T> {
        unsafe { self.sender.as_ref().unwrap() }
    }

    fn wake(&self) {
        unsafe {
            (&mut *self.waker.get()).take().and_then(|w| Some(w.wake()));
        }
    }
}

impl<T: Clone> Unpin for SendFuture<T> {}

impl<T: Clone> core::future::Future for SendFuture<T> {
    type Output = Result<(), SendError>;
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let value = self.value.take().unwrap();
        match self.sender().channel_ref().try_send(value) {
            Ok(()) => Poll::Ready(Ok(())),
            Err(TrySendError::Disconnected(_)) => Poll::Ready(Err(SendError {})),
            Err(TrySendError::Full(msg)) => {
                self.waker = UnsafeCell::new(Some(cx.waker().clone()));
                self.sender().channel_ref().add_waiting_sender(&*self);
                self.value = Some(msg);
                Poll::Pending
            }
        }
    }
}

pub struct Sender<T: Clone> {
    channel: *mut dyn ChannelTrait<T>,
}

impl<T: Clone> Clone for Sender<T> {
    fn clone(&self) -> Self {
        self.channel_ref().add_sender();
        Self {
            channel: self.channel,
        }
    }
}

impl<T: Clone> Drop for Sender<T> {
    fn drop(&mut self) {
        self.channel_ref().drop_sender();
    }
}

impl<'a, T: Clone> Sender<T> {
    fn channel_ref<'b>(&'b self) -> &'b dyn ChannelTrait<T> {
        unsafe { self.channel.as_ref().unwrap() }
    }

    pub fn capacity(&self) -> usize {
        self.channel_ref().capacity()
    }

    pub async fn send(&self, message: T) -> Result<(), SendError> {
        SendFuture {
            sender: self,
            value: Some(message),
            waker: UnsafeCell::new(None),
            next: UnsafeCell::new(core::ptr::null()),
        }
        .await
    }
}

struct RecvFuture<T: Clone> {
    receiver: *const Receiver<T>,
    waker: UnsafeCell<Option<Waker>>,
    next: UnsafeCell<*const RecvFuture<T>>,
}

impl<T: Clone> RecvFuture<T> {
    fn receiver(&self) -> &Receiver<T> {
        unsafe { self.receiver.as_ref().unwrap() }
    }

    fn wake(&self) {
        unsafe {
            (&mut *self.waker.get()).take().and_then(|w| Some(w.wake()));
        }
    }
}

impl<T: Clone> Unpin for RecvFuture<T> {}

impl<T: Clone> core::future::Future for RecvFuture<T> {
    type Output = Result<T, RecvError>;
    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        match self.receiver().channel_ref().try_recv() {
            Ok(value) => Poll::Ready(Ok(value)),
            Err(TryRecvError::Disconnected) => Poll::Ready(Err(RecvError {})),
            Err(TryRecvError::Empty) => {
                self.waker = UnsafeCell::new(Some(cx.waker().clone()));
                self.receiver().channel_ref().add_waiting_receiver(&*self);
                Poll::Pending
            }
        }
    }
}

pub struct Receiver<T: Clone> {
    channel: *mut dyn ChannelTrait<T>,
}

impl<T: Clone> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        self.channel_ref().add_receiver();
        Self {
            channel: self.channel,
        }
    }
}

impl<T: Clone> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.channel_ref().drop_receiver();
    }
}

impl<T: Clone> Receiver<T> {
    fn channel_ref(&self) -> &dyn ChannelTrait<T> {
        unsafe { self.channel.as_ref().unwrap() }
    }
    pub fn capacity(&self) -> usize {
        self.channel_ref().capacity()
    }

    pub async fn recv(&self) -> Result<T, RecvError> {
        RecvFuture {
            receiver: self,
            waker: UnsafeCell::new(None),
            next: UnsafeCell::new(core::ptr::null()),
        }
        .await
    }
}

struct ChannelData<'a, T: Clone> {
    buffer: &'a mut [T],
    read_pos: usize,
    write_pos: usize,
    waiting_senders: *const SendFuture<T>,
    waiting_receivers: *const RecvFuture<T>,
    sender_ref_count: i32,
    receiver_ref_count: i32,
}

impl<'a, T: Clone> ChannelData<'a, T> {
    fn wake_one_receiver(&mut self) -> bool {
        if !self.waiting_receivers.is_null() {
            let to_notify = self.waiting_receivers;
            unsafe {
                let to_notify = &*to_notify;
                self.waiting_receivers = *to_notify.next.get();
                to_notify.wake();
            }
            true
        } else {
            false
        }
    }

    fn wake_all_receivers(&mut self) {
        while self.wake_one_receiver() {}
    }

    fn wake_one_sender(&mut self) -> bool {
        if !self.waiting_senders.is_null() {
            let to_notify = self.waiting_senders;
            unsafe {
                let to_notify = &*to_notify;
                self.waiting_senders = *to_notify.next.get();
                to_notify.wake();
            }
            true
        } else {
            false
        }
    }

    fn wake_all_senders(&mut self) {
        while self.wake_one_sender() {}
    }
}

#[macro_export]
macro_rules! channel {
    ($name:ident, $init:expr, $cnt:expr) => {
        let $name = [$init; $cnt];
        pin_utils::pin_mut!($name);
        let $name = uio::sync::Channel::new($name);
        pin_utils::pin_mut!($name);
    };
}

pub struct Channel<'a, T: Clone> {
    data: core::cell::UnsafeCell<ChannelData<'a, T>>,
}

impl<T: Clone> Unpin for Channel<'_, T> {}

impl<'a, T: Clone> Channel<'a, T> {
    pub fn new(buffer: Pin<&'a mut [T]>) -> Self {
        let buffer_mut = unsafe { buffer.get_unchecked_mut() };
        Self {
            data: core::cell::UnsafeCell::new(ChannelData {
                buffer: buffer_mut,
                read_pos: 0,
                write_pos: 0,
                waiting_senders: core::ptr::null(),
                waiting_receivers: core::ptr::null(),
                sender_ref_count: 0,
                receiver_ref_count: 0,
            }),
        }
    }

    pub fn split(self: Pin<&mut Self>) -> (Sender<T>, Receiver<T>) {
        unsafe {
            self.mut_channel_data().sender_ref_count = 1;
            self.mut_channel_data().receiver_ref_count = 1;
        }
        (
            Sender {
                channel: &*self as *const dyn ChannelTrait<T> as *mut dyn ChannelTrait<T>,
            },
            Receiver {
                channel: &*self as *const dyn ChannelTrait<T> as *mut dyn ChannelTrait<T>,
            },
        )
    }

    unsafe fn mut_channel_data(&self) -> &mut ChannelData<'a, T> {
        &mut *self.data.get()
    }

    unsafe fn channel_data(&self) -> &ChannelData<T> {
        &*self.data.get()
    }

    fn capacity(&self) -> usize {
        unsafe { self.channel_data().buffer.len() }
    }

    fn is_empty(&self) -> bool {
        unsafe {
            let data = self.channel_data();
            data.read_pos == data.write_pos
        }
    }

    fn is_full(&self) -> bool {
        unsafe {
            let data = self.channel_data();
            if data.read_pos == 0 {
                // About to wrap around and read is at 0, so we are full!
                data.write_pos == data.buffer.len() - 1
            } else if data.read_pos > data.write_pos {
                // About to catch up to read position, so we are full
                data.write_pos + 1 == data.read_pos
            } else {
                false
            }
        }
    }

    fn is_disconnected(&self) -> bool {
        unsafe {
            let data = self.channel_data();
            if data.sender_ref_count == 0 && self.is_empty() {
                true
            } else if data.receiver_ref_count == 0 {
                true
            } else {
                false
            }
        }
    }

    fn try_send(&self, message: T) -> Result<(), TrySendError<T>> {
        if self.is_disconnected() {
            Err(TrySendError::Disconnected(message))
        } else if self.is_full() {
            Err(TrySendError::Full(message))
        } else {
            unsafe {
                let mut data = self.mut_channel_data();
                let write_pos = data.write_pos;
                data.buffer[write_pos] = message;
                data.write_pos = (write_pos + 1) % data.buffer.len();
                data.wake_one_receiver();
                Ok(())
            }
        }
    }

    fn try_recv(&self) -> Result<T, TryRecvError> {
        if self.is_disconnected() {
            Err(TryRecvError::Disconnected)
        } else if self.is_empty() {
            Err(TryRecvError::Empty)
        } else {
            unsafe {
                let mut data = self.mut_channel_data();
                let read_pos = data.read_pos;
                let retval = Ok(data.buffer[read_pos].clone());
                data.read_pos = (read_pos + 1) % data.buffer.len();
                data.wake_one_sender();
                retval
            }
        }
    }
}

impl<'a, T: Clone> ChannelTrait<T> for Channel<'a, T> {
    fn capacity(&self) -> usize {
        self.capacity()
    }
    fn try_send(&self, value: T) -> Result<(), TrySendError<T>> {
        self.try_send(value)
    }

    fn try_recv(&self) -> Result<T, TryRecvError> {
        self.try_recv()
    }

    fn drop_sender(&self) {
        unsafe {
            self.mut_channel_data().sender_ref_count -= 1;
            if self.channel_data().sender_ref_count < 0 {
                panic!("Channel invariant corruption: sender ref count must be >= 0!");
            }

            if self.channel_data().sender_ref_count == 0 {
                self.mut_channel_data().wake_all_receivers();
            }
        }
    }

    fn drop_receiver(&self) {
        unsafe {
            self.mut_channel_data().receiver_ref_count -= 1;
            if self.channel_data().receiver_ref_count < 0 {
                panic!("Channel invariant corruption: sender ref count must be >= 0!");
            }

            if self.channel_data().receiver_ref_count == 0 {
                self.mut_channel_data().wake_all_senders();
            }
        }
    }
    fn add_waiting_sender(&self, future: &SendFuture<T>) {
        unsafe {
            let fut_next_ref = &mut *future.next.get();
            *fut_next_ref = self.channel_data().waiting_senders;
            self.mut_channel_data().waiting_senders = future;
        }
    }
    fn add_waiting_receiver(&self, future: &RecvFuture<T>) {
        unsafe {
            let fut_next_ref = &mut *future.next.get();
            *fut_next_ref = self.channel_data().waiting_receivers;
            self.mut_channel_data().waiting_receivers = future;
        }
    }
    fn add_sender(&self) {
        unsafe {
            self.mut_channel_data().sender_ref_count += 1;
        }
    }
    fn add_receiver(&self) {
        unsafe {
            self.mut_channel_data().receiver_ref_count += 1;
        }
    }
}

impl<T: Clone> Drop for Channel<'_, T> {
    fn drop(&mut self) {
        unsafe {
            if self.channel_data().sender_ref_count > 0 {
                panic!("Channel dropped with active senders!");
            }
            if self.channel_data().receiver_ref_count > 0 {
                panic!("Channel dropped with active receivers!");
            }
        }
    }
}
