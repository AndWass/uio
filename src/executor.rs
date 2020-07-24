
use std::sync::mpsc::{Sender, Receiver};

pub struct ExecutorItem {
    handler: fn(*mut ()) -> (),
    data: *mut(),
}

impl ExecutorItem {
    pub fn execute(self) {
        let handler = self.handler;
        handler(self.data);
    }
}

pub struct Executor {
    job_sender: Sender<ExecutorItem>,
    job_receiver: Receiver<ExecutorItem>
}

impl Executor {
    pub fn  new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Executor {
            job_sender: sender,
            job_receiver: receiver
        }
    }
    pub fn run(&mut self) -> ! {
        loop {
            if let Some(next_job) = self.job_receiver.recv().ok() {
                next_job.execute();
            }
        }
    }
}