use core::sync::atomic::{AtomicPtr, Ordering};
use core::task::{Context, RawWaker, RawWakerVTable, Waker};

use core::pin::Pin;

use embedded_async::intrusive::intrusive_list;
use crate::task::TaskWaker;

static mut CURRENT_TASK_FLAG: AtomicPtr<crate::task::TaskWaker> = AtomicPtr::new(core::ptr::null_mut());
static mut TASK_LIST: intrusive_list::List<*mut dyn Task> = intrusive_list::List::new();

fn task_list() -> &'static mut intrusive_list::List<*mut dyn Task> {
    unsafe {
        &mut  TASK_LIST
    }
}

/// Trait for all tasks used by the executor.
pub trait Task {
    /// Access to the waker
    fn waker(&self) -> &'static TaskWaker;
    /// Access to the intrusive list node
    fn node(&mut self) -> &mut intrusive_list::Node<*mut dyn Task>;
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
    let waker = task.waker();
    if waker.is_ready_to_poll() {
        waker.clear_ready_to_poll();
        set_current_task_flag(waker);

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
            let mut available_tasks = intrusive_list::List::new();
            task_list().move_to_front_of(&mut available_tasks);
            let task_list = task_list();
            while let Some(next_task) = available_tasks.pop_front() {
                let task = &mut **next_task.owner_mut().expect("");
                match maybe_poll_task(task) {
                    TaskState::NotReady | TaskState::Pending => {
                        let next_task_ptr = next_task as *mut intrusive_list::Link<*mut dyn Task>;
                        let owner = next_task.owner_mut().expect("");
                        task_list.push_link_back(owner, next_task_ptr);
                    },
                    TaskState::Finished => task.waker().set_finished(),
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
    task.waker().set_started();
    task.waker().set_ready_to_poll();
    *task.node() = intrusive_list::Node::new(task as *mut _);
    task_list().push_node_back(task.node());

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
    unsafe { & *CURRENT_TASK_FLAG.load(Ordering::Acquire) }
}

fn make_waker_for_current() -> Waker {
    make_waker(current_task_flag())
}

fn make_waker(flag: &'static TaskWaker) -> Waker {
    let data = flag as *const TaskWaker as *const ();
    unsafe { Waker::from_raw(make_raw_waker(data)) }
}

fn raw_wake(data: *const ()) {
    if data.is_null() {
        return;
    }

    let data = unsafe { &*(data as *const TaskWaker) };
    data.set_ready_to_poll();
}

fn make_raw_waker(data: *const ()) -> RawWaker {
    static WAKER_VTABLE: RawWakerVTable =
        RawWakerVTable::new(make_raw_waker, raw_wake, raw_wake, |_| ());
    RawWaker::new(data, &WAKER_VTABLE)
}
