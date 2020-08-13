use core::future::Future;
use core::pin::Pin;
use core::task::Poll::*;
use core::task::{Context, Poll};
use std::thread::spawn;

#[macro_use]
extern crate lazy_static;

pub struct Delay {
    sleep_time: u32,
    done: bool,
}

impl Delay {
    pub fn new(sleep_time_sec: u32) -> Self {
        Self {
            sleep_time: sleep_time_sec,
            done: false,
        }
    }
}

impl Future for Delay {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if self.done {
            Ready(())
        } else {
            let sleep_time = self.sleep_time;
            let mut_self = unsafe { self.get_unchecked_mut() };
            mut_self.done = true;
            let waker = cx.waker().clone();
            spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(sleep_time.into()));
                waker.wake();
            });
            Poll::Pending
        }
    }
}

struct StdinSerial {}

impl uio::serial::PollRead for StdinSerial {
    type Error = std::io::Error;

    fn poll_read(
        &mut self,
        waker: uio::interrupt::WakerRef,
        buffer: &mut [u8],
    ) -> Poll<Result<usize, Self::Error>> {
        let mut locked = STDIN_BUFFER.lock().unwrap();
        let stdin_amount = locked.len();
        if stdin_amount > 0 {
            let amount_to_read = stdin_amount.min(buffer.len());
            buffer[..amount_to_read].copy_from_slice(&locked.as_bytes()[..amount_to_read]);
            *locked = String::new();
            Poll::Ready(Ok(amount_to_read))
        } else {
            spawn(move || {
                let mut line = String::new();
                println!("Write something:");
                let _result = std::io::stdin().read_line(&mut line);
                let mut stdin_data = STDIN_BUFFER.lock().unwrap();
                *stdin_data = line;
                waker.try_wake();
            });
            Poll::Pending
        }
    }
}

lazy_static! {
    static ref STDIN_BUFFER: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
}

async fn read_stdin<'a>(mut serial: impl uio::serial::AsyncRead) {
    println!("Reading stdin");
    let mut buffer = [0u8; 32];
    match uio::serial::read(&mut serial, &mut buffer).await {
        Ok(()) => {
            println!("Read '{}'", String::from_utf8_lossy(&buffer).trim());
        }
        Err((amount, _)) => {
            println!("Read error after {} bytes", amount);
        }
    }
}

async fn task_2(wait_time: u32) -> i32 {
    println!("Waiting in task_2");
    let start_time = std::time::SystemTime::now();
    Delay::new(wait_time).await;
    let elapsed = std::time::SystemTime::now()
        .duration_since(start_time)
        .unwrap();
    assert!(elapsed.as_millis() as u32 >= wait_time * 1000 - 100);
    println!("task_2 done");
    elapsed.as_millis() as i32
}

async fn task_1() {
    let mut task2 = uio::task::Task::new(task_2(2));
    uio::executor::start(&mut task2);

    static STDIN_WAKER: uio::interrupt::Waker = uio::interrupt::Waker::new();
    let mut stdin_task = uio::task::Task::new(read_stdin(uio::serial::ReaderWithWaker::new(
        StdinSerial {},
        &STDIN_WAKER,
    )));
    uio::executor::start(&mut stdin_task);

    println!("Task 2 took {} milliseconds", task2.join_handle().await);
    stdin_task.join_handle().await;
}

fn main() {
    let mut main_task = uio::task::Task::new(task_1());
    uio::executor::start(&mut main_task);
    uio::executor::run();
}
