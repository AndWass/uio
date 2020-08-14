use core::future::Future;
use core::pin::Pin;
use core::task::Poll::*;
use core::task::{Context, Poll};
use std::thread::spawn;

pub struct Producer {
    value: Option<u32>,
}

impl Unpin for Producer {}

impl Producer {
    pub fn new() -> Self {
        Self {
            value: None
        }
    }
}

impl Future for Producer {
    type Output = u32;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if self.value.is_some() {
            Ready(self.value.take().unwrap())
        } else {
            let generated_value = rand::random();
            self.value = Some(generated_value);
            let sleep_time = generated_value & 0x00FF00 >> 8;
            let waker = cx.waker().clone();
            spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(sleep_time as u64));
                waker.wake();
            });
            Poll::Pending
        }
    }
}

#[derive(Copy, Clone)]
struct Job {
    producer: u32,
    value: u32
}

impl Job {
    pub const fn new() -> Self {
        Self {
            producer: 0,
            value: 0
        }
    }
}

struct Delay {
    millis: Option<u32>
}

impl Unpin for Delay {}
impl core::future::Future for Delay {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.millis.is_none() {
            Poll::Ready(())
        }
        else {
            let sleep_time = self.millis.take().unwrap();
            let waker = cx.waker().clone();
            spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(sleep_time as u64));
                waker.wake();
            });
            Poll::Pending
        }
    }
    
}

async fn task_sleep(time: u32) {
    Delay {
        millis: Some(time)
    }.await;
}

async fn consumer(id: u32, receiver: uio::sync::Receiver<Job>)
{
    let mut job_count = 0i32;
    'main_loop: loop {
        match receiver.recv().await {
            Ok(job) => {
                job_count += 1;
                println!("CONSUMER {} received {} from {}", id, job.value, job.producer);
                println!("CONSUMER {} sleeping for {}ms", id, job.value & 0x03FF);
                task_sleep(job.value & 0x00FF).await;
            },
            Err(_) => {
                println!("CONSUMER {} stopping", id);
                break 'main_loop;
            }
        }
    }
    println!("{} consumed {} jobs", id, job_count);
}

async fn producer(id: u32, sender: uio::sync::Sender<Job>)
{
    for _ in 0..5 {
        let value = Producer{
            value: None
        }.await;
        println!("PRODUCER {} producing {}", id, value);
        match sender.send(Job {
            producer: id,
            value
        }).await {
            Ok(_) => {

            },
            Err(_) => {
                println!("ERROR producing from {}, stopping!", id);
                return;
            }
        }
    }

    println!("PRODUCER {} stopping", id);
}

fn main() {
    use uio::{task_start, channel};
    // Create a channel
    channel!(job_channel, Job::new(), 10);
    // Get a sender and receiver to the channel
    let (s1, recv1) = job_channel.split();
    
    task_start!(producer1, producer(1, s1.clone()));
    task_start!(producer2, producer(2, s1.clone()));
    task_start!(producer3, producer(3, s1.clone()));
    task_start!(producer4, producer(4, s1.clone()));
    task_start!(producer5, producer(5, s1.clone()));
    task_start!(producer6, producer(6, s1));

    task_start!(consumer1, consumer(1, recv1.clone()));
    task_start!(consumer2, consumer(2, recv1));

    uio::executor::run();
}
