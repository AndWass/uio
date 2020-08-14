async fn hello_world() {
    println!("Hello world!");
}

fn main() {
    println!("Starting task");
    uio::task_start!(main_task, hello_world());
    println!("Running executor");
    uio::executor::run();
}
