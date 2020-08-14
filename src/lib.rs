#![no_std]

//! This library contains an async executor targeting `no_std` embedded boards without
//! any allocator support.
//!
//! This is done in an intrusive approach where tasks must implement the `executor::Task` trait.
//!
//! ## Example
//!
//! ```
//! use core::future::Future;
//! use core::task::{Context, Poll};
//! use core::task::Poll::*;
//! use core::pin::Pin;
//!
//! use std::thread::spawn;
//! # pub struct Delay {
//! #     sleep_time: u32,
//! #     done: bool,
//! # }
//! #
//! # impl Delay{
//! #     pub fn new(sleep_time_sec: u32) -> Self {
//! #         Self {
//! #             sleep_time: sleep_time_sec,
//! #             done: false
//! #         }
//! #     }
//! # }
//! #
//! # impl Future for Delay {
//! #     type Output = ();
//! #
//! #     fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
//! #         if self.done {
//! #             Ready(())
//! #         }
//! #         else {
//! #             let sleep_time = self.sleep_time;
//! #             self.done = true;
//! #             let waker = cx.waker().clone();
//! #             spawn(move || {
//! #                 std::thread::sleep(std::time::Duration::from_secs(sleep_time.into()));
//! #                 waker.wake();
//! #             });
//! #             Pending
//! #         }
//! #     }
//! # }
//!
//! async fn task_2(wait_time: u32) -> i32 {
//!     println!("Waiting in task_2");
//!     let start_time = std::time::SystemTime::now();
//!     Delay::new(wait_time).await;
//!     let elapsed = std::time::SystemTime::now().duration_since(start_time).unwrap();
//!     assert!(elapsed.as_millis() as u32 >= wait_time*1000 - 100);
//!     println!("task_2 done");
//!     elapsed.as_millis() as i32
//! }
//!
//! async fn task_1() {
//!     uio::task_start!(task2, task_2(2));
//!
//!     println!("Task 2 took {} milliseconds", task2.join().await);
//! }
//!
//! fn main() {
//!     // Creation and starting of tasks are decoupled to allow for intrusive
//!     // handling of tasks.
//!     uio::task_start!(main_task, task_1());
//!     uio::executor::run();
//! }
//! ```

pub use pin_utils;

/// I/O traits and helpers.
pub mod digital;
/// Executor types, traits and functions.
pub mod executor;
/// Asynchronous values.
pub mod future;
/// Interrupt traits and helpers.
pub mod interrupt;
/// Serial I/O traits and helpers.
pub mod serial;
pub mod sync;
/// Types for working with tasks.
pub mod task;
/// Re-export embedded hal
pub use embedded_hal;

mod task_list;
