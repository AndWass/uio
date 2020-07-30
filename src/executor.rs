
use core::task::{Waker, RawWaker, RawWakerVTable, Context};
use core::sync::atomic::{AtomicBool, Ordering, AtomicPtr};

use core::pin::Pin;
use core::mem::MaybeUninit;
use core::task::Poll;

static mut CURRENT_TASK_FLAG: AtomicPtr<AtomicBool> = AtomicPtr::new(core::ptr::null_mut());
static mut TASK_LIST: MaybeUninit<TaskList> = MaybeUninit::uninit();

struct EmptyFuture {}

impl core::future::Future for EmptyFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(())
    }
}

fn task_list() -> &'static mut TaskList {
    static mut TASK_LIST_INIT: bool = false;
    unsafe {
        if !TASK_LIST_INIT {
            TASK_LIST = MaybeUninit::new(TaskList::new());
            TASK_LIST_INIT = true;
        }

        &mut *TASK_LIST.as_mut_ptr()
    }
}

struct TaskList {
    head: *mut dyn Task,
}

impl TaskList {
    fn end_item() -> *mut dyn Task {
        static mut END_TASK: MaybeUninit<crate::task::Task<(), EmptyFuture>> = MaybeUninit::uninit();
        static mut END_INIT: bool = false;

        unsafe {
            if !END_INIT {
                END_INIT = true;
                END_TASK = MaybeUninit::new(crate::task::Task::new(EmptyFuture{}));
            }
            END_TASK.as_mut_ptr()
        }
    }
    fn new() -> Self {
        let null: *mut dyn Task = Self::end_item();
        Self {
            head: null,
        }
    }

    fn is_empty(&self) -> bool {
        self.head == Self::end_item()
    }

    fn push_front(&mut self, item: *mut dyn Task) {
        let item = unsafe { &mut *item };
        item.mut_task_data().next = self.head;
        self.head = item;
    }

    fn pop_front(&mut self) -> *mut dyn Task {
        let retval = unsafe { &mut *self.head };
        self.head = retval.mut_task_data().next;
        retval
    }

    fn take(&mut self) -> Self {
        let retval = core::mem::replace(&mut self.head, Self::end_item());
        Self {
            head: retval,
        }
    }

    fn merge(&mut self, mut other: TaskList) {
        if self.is_empty() {
            self.head = other.head;
        }
        else {
            while !other.is_empty() {
                self.push_front(other.pop_front());
            }
        }
    }
}

pub struct TaskData {
    ready_flag: AtomicBool,
    next: *mut dyn Task,
}

impl TaskData {
    pub fn new() -> Self {
        Self {
            ready_flag: AtomicBool::new(false),
            next: TaskList::end_item(),
        }
    }
}

pub trait Task {
    fn mut_task_data(&mut self) -> &mut TaskData;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> core::task::Poll<()>;
}

#[derive(PartialOrd, PartialEq)]
enum TaskResult {
    NotReady,
    Pending,
    Finished
}

fn maybe_poll_task(task: &mut (dyn Task + 'static)) -> TaskResult {
    let task_data = task.mut_task_data();
    if task_data.ready_flag.load(Ordering::Acquire) {
        task_data.ready_flag.store(false, Ordering::Release);
        set_current_task_flag(&mut task_data.ready_flag);

        let pinned_fut = unsafe { core::pin::Pin::new_unchecked(task) };
        let waker = make_waker_for_current();
        let mut context = Context::from_waker(&waker);
        match pinned_fut.poll(&mut context) {
            core::task::Poll::Ready(_) => TaskResult::Finished,
            _ => TaskResult::Pending
        }
    }
    else {
        TaskResult::NotReady
    }
}

pub fn run() {
    'main_loop: loop {
        run_one();

        if task_list().is_empty() {
            break 'main_loop;
        }
    }
}

pub fn run_one() {
    unsafe {
        let mut available_tasks =  task_list().take();
        while !available_tasks.is_empty() {
            let next_task = available_tasks.pop_front();
            let task_result = maybe_poll_task(&mut *next_task);
            if task_result == TaskResult::NotReady {
                task_list().push_front(next_task);
            }
            else if task_result == TaskResult::Finished {
                task_list().merge(available_tasks);
                break;
            }
            else if task_result == TaskResult::Pending {
                task_list().push_front(next_task);
                task_list().merge(available_tasks);
                break;
            }
        }
    }
}

pub fn start(task: &mut (dyn Task + 'static)) {
    task.mut_task_data().ready_flag.store(true, Ordering::Release);
    task_list().push_front(task);
}

fn set_current_task_flag(flag: &mut AtomicBool) {
    unsafe {
        CURRENT_TASK_FLAG.store(flag as *mut AtomicBool, Ordering::Release);
    }
}

fn current_task_flag<'a>() -> &'a mut AtomicBool {
    unsafe { &mut *CURRENT_TASK_FLAG.load(Ordering::Acquire) }
}

fn make_waker_for_current() -> Waker {
    make_waker(current_task_flag())
}

fn make_waker(flag: &mut AtomicBool) -> Waker {
    let data = flag as *const AtomicBool as *const ();
    unsafe { Waker::from_raw(make_raw_waker(data)) }
}

fn raw_wake(data: *const ()) {
    if data.is_null() {
        return;
    }

    let data = data as *mut () as *mut AtomicBool;
    unsafe { &mut (*data).store(true, Ordering::Release); }
}

fn make_raw_waker(data: *const ()) -> RawWaker {
    static WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(make_raw_waker, raw_wake, raw_wake, |_|());
    RawWaker::new(data, &WAKER_VTABLE)
}