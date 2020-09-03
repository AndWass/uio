async fn async_fn() {
    println!("First usage!");
}

#[test]
fn reuse_after_drop() {
    static NEXT_WAKER: uio::task::TaskWaker = uio::task::TaskWaker::new();
    {
        let _next_task = uio::task::Task::new(async_fn(), &NEXT_WAKER);
    }
    let _next_task = uio::task::Task::new(async_fn(), &NEXT_WAKER);
}

#[test]
#[should_panic]
fn panic_on_reuse() {
    static NEXT_WAKER: uio::task::TaskWaker = uio::task::TaskWaker::new();
    let _next_task = uio::task::Task::new(async_fn(), &NEXT_WAKER);
    let _next_task = uio::task::Task::new(async_fn(), &NEXT_WAKER);
}
