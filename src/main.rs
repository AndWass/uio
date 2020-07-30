use crate::scratch::IrqFuture;

mod executor;
mod task;
mod scratch;
mod future_value;

async fn test2() -> i32 {
    println!("Hello from 2");
    IrqFuture::new_with_sleep(2).await;
    println!("World form 2");
    10
}

async fn test() {
    println!("Hello world!");
    let mut test2_task = task::Task::new(test2());
    executor::start(&mut test2_task);
    println!("{}", test2_task.join_handle().await);
    println!("Waiting irqfuture");
    let fut = IrqFuture::new();
    fut.await;
    println!("Weee");
    let fut = IrqFuture::new();
    fut.await;
    println!("Wooo");
}

fn main() {
    let mut main_task = task::Task::new(test());
    println!("main_task size = {}", core::mem::size_of_val(&main_task));
    executor::start(&mut main_task);
    executor::run();
}
