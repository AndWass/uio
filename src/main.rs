mod executor;
mod scratch;

async fn test() {
    println!("Hello world!");
}

fn main() {
    let mut exec = executor::Executor::new();
    let _fut = test();
    //exec.run();
}
