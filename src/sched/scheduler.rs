use hal::common::{sched::{CtxPtr, ThreadDesc}, sync::SpinLocked};

use crate::mem::{self, alloc::AllocError, array::IndexMap, heap::PriorityQueue, queue::Queue};

use super::task::{Task, TaskDesc, TaskId, Thread, ThreadId, ThreadState, Timing};

pub static SCHEDULER: SpinLocked<Scheduler> = SpinLocked::new(Scheduler::new());

/// TODO: Make this dynamic.
pub const MAX_THREADS: usize = 32;

pub struct Scheduler<'a> {
    current: Option<ThreadId>,
    // Fast interval store.
    current_interval: usize,
    tasks: IndexMap<Task<'a>, 8>,
    threads: IndexMap<Thread, 32>,
    queue: PriorityQueue<'a, (ThreadId, usize)>,
    callbacks: Queue<(ThreadId, usize), 32>,
    time: usize,
}

impl Scheduler<'_> {
    pub const fn new() -> Self {
        Self {
            current: None,
            current_interval: 0,
            tasks: IndexMap::new(),
            threads: IndexMap::new(),
            queue: PriorityQueue::new(),
            callbacks: Queue::new(),
            time: 0
        }
    }

    pub fn create_task(&mut self, desc: TaskDesc, main_desc: ThreadDesc, main_timing: Timing) -> Result<TaskId, AllocError> {
        let size = mem::align_up(desc.mem_size) + mem::align_up(desc.stack_size);
        let mut task = Task::new(size)?;

        let period = main_timing.period;

        let thread_ctx = task.create_thread_ctx(main_desc)?;
        let thread = Thread::new(thread_ctx, main_timing);

        let thread_id = self.threads.insert_next(thread)?;
        task.register_thread(thread_id)?;

        let task_id = self.tasks.insert_next(task)?;

        self.queue.push((thread_id, period));

        if let Some(task) = self.tasks.get_mut(task_id) {
            task.id = task_id.into();
            return Ok(task_id.into());
        }

        
        Err(AllocError::OutOfMemory)
    }

    fn update_current_ctx(&mut self, ctx: CtxPtr) {
        if let Some(id) = self.current {
            if let Some(thread) = self.threads.get_mut(id) {
                thread.context = ctx.into();
            }
        }
    }

    fn select_new_thread(&mut self) -> Option<CtxPtr> {
        if let Some(id) = self.queue.pop().map(|(id, _)| id) {
            // Set the previous thread as ready. And add a callback from now.
            if let Some(id) = self.current {
                if let Some(thread) = self.threads.get_mut(id) {
                    thread.state = ThreadState::Ready;
                    // The delay that is already in the queue.
                    let delay = self.callbacks.back().map(|(_, delay)| *delay).unwrap_or(0);
                    // Add the callback to the queue.
                    if thread.period - self.time + delay > 0 {
                        self.callbacks.push_back((id, thread.period - self.time + delay));
                    } else {
                        self.queue.push((id, thread.period));
                    }
                }
            }

            if let Some(thread) = self.threads.get_mut(id) {
                thread.state = ThreadState::Runs;

                // Set the new thread as the current one.
                self.current_interval = thread.deadline;
                self.current = Some(id);

                // Return the new thread context.
                return Some(thread.context.into());
            }
        }

        None
    }

    fn fire_thread_if_necessary(&mut self) -> bool {
        let mut found = false;
        loop {
            if let Some((id, cnt)) = self.callbacks.front().cloned() {
                if cnt - 1 == 0 {
                    self.callbacks.pop_front();
                    if let Some(thread) = self.threads.get_mut(id) {
                        thread.state = ThreadState::Ready;
                        self.queue.push((id, thread.deadline));
                        found = true;
                    }
                } else {
                    self.callbacks.insert(0, (id, cnt - 1));
                    break;
                }
            } else {
                break;
            }
        }

        found
    }

    fn tick(&mut self) -> bool {
        self.time += 1;

        if self.fire_thread_if_necessary() {
            return true;
        }

        if self.time >= self.current_interval {
            self.time = 0;
            return true;
        }

        false
    }
}

/// cbindgen:ignore
/// cbindgen:no-export
#[no_mangle]
pub extern "C" fn sched_enter(ctx: CtxPtr) -> CtxPtr {
    hal::hprintln!("Scheduler resched.").unwrap();

    {
        let mut scheduler = SCHEDULER.lock();

        scheduler.update_current_ctx(ctx);
        scheduler.select_new_thread().unwrap_or(ctx)
    }
}

/// cbindgen:ignore
/// cbindgen:no-export
#[no_mangle]
pub extern "C" fn systick() {
    let resched = {
        let mut scheduler = SCHEDULER.lock();
        scheduler.tick()
    };

    if resched {
        hal::common::sched::reschedule();
    }
}



