use core::sync::atomic::{AtomicPtr, Ordering};
use core::task::{Context, RawWaker, RawWakerVTable, Waker};

use core::mem::MaybeUninit;
use core::pin::Pin;

use crate::task_list::TaskList;
use crate::task::TaskWaker;

static mut CURRENT_TASK_FLAG: AtomicPtr<crate::task::TaskWaker> = AtomicPtr::new(core::ptr::null_mut());
static mut TASK_LIST: MaybeUninit<TaskList> = MaybeUninit::uninit();

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

/// Trait for all tasks used by the executor.
pub trait Task {
    /// Function to mutably access TaskData that must be stored by any task implementation.
    fn mut_task_data(&mut self) -> &mut crate::task::TaskData;
    /// Poll the task. This will normally delegate to some stored futures poll function.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> core::task::Poll<()>;
}

pub struct TaskResult<T> {
    value: *mut crate::future::Value<T>,
}

impl<T: Unpin> TaskResult<T> {
    pub async fn join(self) -> T {
        unsafe {
            let value = &mut *self.value;
            value.await
        }
    }
}

pub trait TypedTask: Task {
    type Output;

    fn value_ptr(&mut self) -> *mut crate::future::Value<Self::Output>;
}

#[derive(PartialOrd, PartialEq)]
enum TaskState {
    NotReady,
    Pending,
    Finished,
}

fn maybe_poll_task(task: &mut (dyn Task + 'static)) -> TaskState {
    let task_data = task.mut_task_data();
    if task_data.is_ready_to_poll() {
        task_data.waker.clear_ready_to_poll();
        set_current_task_flag(&task_data.waker);

        let task = unsafe { core::pin::Pin::new_unchecked(task) };
        let waker = make_waker_for_current();
        let mut context = Context::from_waker(&waker);
        match task.poll(&mut context) {
            core::task::Poll::Ready(_) => TaskState::Finished,
            _ => TaskState::Pending,
        }
    } else {
        TaskState::NotReady
    }
}

/// Runs the executor until all started and will-be-started tasks have finished.
///
/// This is typically only called once at the end of main.
///
/// # Panics
///
/// Any nested calls will cause a panic.
pub fn run() {
    static mut TAKEN: bool = false;
    unsafe {
        if TAKEN {
            panic!("Nested calls to run not supported");
        }
        TAKEN = true;
    }
    'main_loop: loop {
        unsafe {
            let mut available_tasks = task_list().take();
            let task_list = task_list();
            while !available_tasks.is_empty() {
                let next_task = available_tasks.pop_front();
                match maybe_poll_task(&mut *next_task) {
                    TaskState::NotReady | TaskState::Pending => task_list.push_front(next_task),
                    TaskState::Finished => (&mut *next_task).mut_task_data().waker.set_finished(),
                }
            }
        }
        if task_list().is_empty() {
            break 'main_loop;
        }
    }
    unsafe {
        TAKEN = false;
    }
}

/// Start a task, scheduling it to be run.
///
/// This can be called both before `run()` and within async functions.
///
/// # Arguments
///
/// * `task` - The task to start.
pub fn start<T: Task + TypedTask + 'static>(task: Pin<&mut T>) -> TaskResult<T::Output> {
    let task = unsafe { task.get_unchecked_mut() };
    task.mut_task_data().waker.set_started();
    task.mut_task_data().waker.set_ready_to_poll();
    task_list().push_front(&mut *task);

    TaskResult {
        value: task.value_ptr(),
    }
}

fn set_current_task_flag(waker: &'static TaskWaker) {
    unsafe {
        CURRENT_TASK_FLAG.store(waker as *const TaskWaker as *mut TaskWaker, Ordering::Release);
    }
}

fn current_task_flag() -> &'static TaskWaker {
    unsafe { &mut *CURRENT_TASK_FLAG.load(Ordering::Acquire) }
}

fn make_waker_for_current() -> Waker {
    make_waker(current_task_flag())
}

fn make_waker(waker: &'static TaskWaker) -> Waker {
    let data = waker as *const TaskWaker as *const ();
    unsafe { Waker::from_raw(make_raw_waker(data)) }
}

fn raw_wake(data: *const ()) {
    if data.is_null() {
        return;
    }

    let data = data as *mut () as *mut TaskWaker;
    unsafe { &mut *data }.set_ready_to_poll();
}

fn make_raw_waker(data: *const ()) -> RawWaker {
    static WAKER_VTABLE: RawWakerVTable =
        RawWakerVTable::new(make_raw_waker, raw_wake, raw_wake, |_| ());
    RawWaker::new(data, &WAKER_VTABLE)
}
