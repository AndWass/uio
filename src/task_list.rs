use crate::executor::{Task, TaskWaker};

use core::mem::MaybeUninit;

use core::pin::Pin;
use core::task::{Context, Poll};

struct EmptyFuture {}

impl core::future::Future for EmptyFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(())
    }
}

pub struct TaskList {
    head: *mut dyn Task,
}

impl TaskList {
    pub fn end_item() -> *mut dyn Task {
        static mut END_TASK: MaybeUninit<crate::task::Task<EmptyFuture>> = MaybeUninit::uninit();
        static mut END_INIT: bool = false;
        static WAKER: TaskWaker = TaskWaker::new();

        unsafe {
            if !END_INIT {
                END_INIT = true;
                END_TASK = MaybeUninit::new(crate::task::Task::new(EmptyFuture {}, &WAKER));
            }
            END_TASK.as_mut_ptr()
        }
    }
    pub fn new() -> Self {
        let null: *mut dyn Task = Self::end_item();
        Self { head: null }
    }

    pub fn is_empty(&self) -> bool {
        self.head == Self::end_item()
    }

    pub fn push_front(&mut self, item: *mut dyn Task) {
        let item = unsafe { &mut *item };
        item.mut_task_data().next = self.head;
        self.head = item;
    }

    pub fn pop_front(&mut self) -> *mut dyn Task {
        let retval = unsafe { &mut *self.head };
        self.head = retval.mut_task_data().next;
        retval
    }

    pub fn take(&mut self) -> Self {
        let retval = core::mem::replace(&mut self.head, Self::end_item());
        Self { head: retval }
    }

    #[allow(dead_code)]
    pub fn merge(&mut self, mut other: TaskList) {
        if self.is_empty() {
            self.head = other.head;
        } else {
            while !other.is_empty() {
                self.push_front(other.pop_front());
            }
        }
    }
}
