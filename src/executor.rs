use core::sync::atomic::{AtomicPtr, AtomicU8, Ordering};
use core::task::{Context, RawWaker, RawWakerVTable, Waker};

use core::mem::MaybeUninit;
use core::ops::{BitAnd, BitOr};
use core::pin::Pin;

use crate::task_list::TaskList;

static mut CURRENT_TASK_FLAG: AtomicPtr<TaskWaker> = AtomicPtr::new(core::ptr::null_mut());
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

pub struct TaskWaker {
    ready_flag: AtomicU8,
}

impl TaskWaker {
    pub const fn new() -> Self {
        Self {
            ready_flag: AtomicU8::new(0)
        }
    }

    fn update_flag(&self, update_fn: impl Fn(u8) -> u8) -> u8 {
        let mut flag_value = self.ready_flag.load(Ordering::Acquire);
        let mut new_value = update_fn(flag_value);
        while self.ready_flag.compare_and_swap(flag_value, new_value, Ordering::SeqCst) != flag_value {
            flag_value = self.ready_flag.load(Ordering::Acquire);
            new_value = update_fn(flag_value);
        }

        flag_value
    }

    fn set_started(&self) {
        self.update_flag(|value| value.bitor(0b0000_0010));
    }

    fn set_finished(&self) {
        self.update_flag(|value| value.bitand(0b1111_1101));
    }

    fn set_ready_to_poll(&self) {
        self.update_flag(|value| value.bitor(0x01));
    }

    fn clear_ready_to_poll(&self) {
        self.update_flag(|value| value.bitand(0b1111_1110));
    }

    pub(crate) fn try_take_reference(&self) -> bool {
        let mut flag_value = self.ready_flag.load(Ordering::Acquire);
        if flag_value & 0b0000_0100 != 0 {
            return false;
        }
        let mut new_value = flag_value | 0b0000_0100;
        while self.ready_flag.compare_and_swap(flag_value, new_value, Ordering::SeqCst) != flag_value {
            flag_value = self.ready_flag.load(Ordering::Acquire);
            if flag_value & 0b0000_0100 != 0 {
                return false;
            }
            new_value = flag_value | 0b0000_0100;
        }
        return true;
    }

    pub(crate) fn release_reference(&self) -> bool {
        let mut flag_value = self.ready_flag.load(Ordering::Acquire);
        if flag_value & 0b0000_0100 == 0 {
            return false;
        }

        let mut new_value = flag_value & !0b0000_0100;

        while self.ready_flag.compare_and_swap(flag_value, new_value, Ordering::SeqCst) != flag_value {
            flag_value = self.ready_flag.load(Ordering::Acquire);
            if flag_value & 0b0000_0100 == 0 {
                return false;
            }
            new_value = flag_value & !0b0000_0100;
        }
        return true;
    }
}

/// Any tasks type used by the executor must store an instance of this structure. It must not be changed
/// by the task once the task has been started.
///
/// It is used internally by the executor and cannot be used in any other way.
///
/// # Panics
///
/// Panics if any instance of TaskData is dropped while the owning task is still active within the
/// executor.
pub struct TaskData {
    waker: &'static TaskWaker,
    pub(crate) next: *mut dyn Task,
}

impl TaskData {
    /// Constructs a new task-data instance. This is the only function available.
    /// Only used when constructing task-objects.
    pub fn new(waker: &'static TaskWaker) -> Self {
        if !waker.try_take_reference() {
            panic!("Attempting to reuse waker already in use by a different task.");
        }
        Self {
            waker,
            next: TaskList::end_item(),
        }
    }

    fn is_ready_to_poll(&self) -> bool {
        (self.waker.ready_flag.load(Ordering::Acquire) & 0x01) > 0
    }
}

/// If TaskData is in use by the executor (started but not finished, ready for poll etc.)
/// when it is dropped the drop code will panic.
impl Drop for TaskData {
    fn drop(&mut self) {
        if !self.waker.release_reference() {
            panic!("Releasing an already released reference!");
        }
        if (self.waker.ready_flag.load(Ordering::Acquire) & 0b0000_0010) != 0 {
            panic!("Task dropped while it is still running");
        }
    }
}

/// Trait for all tasks used by the executor.
pub trait Task {
    /// Function to mutably access TaskData that must be stored by any task implementation.
    fn mut_task_data(&mut self) -> &mut TaskData;
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
