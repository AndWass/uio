use core::future::Future;
use core::task::{Context, Poll};
use core::task::Poll::*;
use core::pin::Pin;

use std::thread::spawn;

pub struct Delay {
    sleep_time: u32,
    done: bool,
}

impl Delay{
    pub fn new(sleep_time_sec: u32) -> Self {
        Self {
            sleep_time: sleep_time_sec,
            done: false
        }
    }
}

impl Future for Delay {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if self.done {
            Ready(())
        }
        else {
            let sleep_time = self.sleep_time;
            let mut_self = unsafe { self.get_unchecked_mut() };
            mut_self.done = true;
            let waker = cx.waker().clone();
            spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(sleep_time.into()));
                waker.wake();
            });
            Pending
        }
    }
}

async fn task_2(wait_time: u32) -> i32 {
    println!("Waiting in task_2");
    let start_time = std::time::SystemTime::now();
    Delay::new(wait_time).await;
    let elapsed = std::time::SystemTime::now().duration_since(start_time).unwrap();
    assert!(elapsed.as_millis() as u32 >= wait_time*1000 - 100);
    println!("task_2 done");
    elapsed.as_millis() as i32
}

async fn task_1() {
    let mut task2 = uio::task::Task::new(task_2(2));
    uio::executor::start(&mut task2);

    println!("Task 2 took {} milliseconds", task2.join_handle().await);
}

fn main() {
    let mut main_task = uio::task::Task::new(task_1());
    uio::executor::start(&mut main_task);
    uio::executor::run();
}
