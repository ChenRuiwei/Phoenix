//! Adapted from Titanix

#![no_std]
#![no_main]

extern crate alloc;

use alloc::collections::VecDeque;
use core::future::Future;

use async_task::{Runnable, ScheduleInfo, Task, WithInfo};
use sync::mutex::SpinNoIrqLock;

static TASK_QUEUE: TaskQueue = TaskQueue::new();

struct TaskQueue {
    normal: SpinNoIrqLock<VecDeque<Runnable>>,
    prior: SpinNoIrqLock<VecDeque<Runnable>>,
}

impl TaskQueue {
    pub const fn new() -> Self {
        Self {
            normal: SpinNoIrqLock::new(VecDeque::new()),
            prior: SpinNoIrqLock::new(VecDeque::new()),
        }
    }

    pub fn push_normal(&self, runnable: Runnable) {
        self.normal.lock().push_back(runnable);
    }

    pub fn push_prior(&self, runnable: Runnable) {
        self.prior.lock().push_back(runnable);
    }

    pub fn fetch_normal(&self) -> Option<Runnable> {
        self.normal.lock().pop_front()
    }

    pub fn fetch_prior(&self) -> Option<Runnable> {
        self.prior.lock().pop_front()
    }

    pub fn fetch(&self) -> Option<Runnable> {
        self.prior
            .lock()
            .pop_front()
            .or_else(|| self.normal.lock().pop_front())
    }

    pub fn len(&self) -> usize {
        self.prior_len() + self.normal_len()
    }

    pub fn prior_len(&self) -> usize {
        self.prior.lock().len()
    }

    pub fn normal_len(&self) -> usize {
        self.normal.lock().len()
    }
}

/// Add a task into task queue
pub fn spawn<F>(future: F) -> (Runnable, Task<F::Output>)
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let schedule = move |runnable: Runnable, info: ScheduleInfo| {
        if info.woken_while_running {
            // i.e `yield_now()`
            TASK_QUEUE.push_normal(runnable);
        } else {
            // i.e. woken up by some signal
            TASK_QUEUE.push_prior(runnable);
        }
    };
    async_task::spawn(future, WithInfo(schedule))
}

pub fn run_until_idle() -> usize {
    let mut len = 0;
    while let Some(task) = TASK_QUEUE.fetch() {
        task.run();
        len += 1
    }
    len
}

pub fn run_one() {
    if let Some(task) = TASK_QUEUE.fetch() {
        task.run();
    }
}

pub fn run_prior_until_idle() {
    while let Some(task) = TASK_QUEUE.fetch_prior() {
        task.run();
    }
}

pub fn has_task() -> bool {
    TASK_QUEUE.len() >= 1
}

pub fn has_prior_task() -> bool {
    TASK_QUEUE.prior_len() >= 1
}

pub fn task_len() -> usize {
    TASK_QUEUE.len()
}
