use alloc::{
    boxed::Box,
    collections::BTreeMap,
    string::{String, ToString},
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    cell::SyncUnsafeCell,
    sync::atomic::{AtomicI8, AtomicUsize},
    task::Waker,
};

use log::debug;
use sync::mutex::SpinNoIrqLock;

use super::pid::PidHandle;
use crate::{
    mm::MemorySpace,
    stack_trace,
    task::{manager::TASK_MANAGER, pid::alloc_pid},
    trap::TrapContext,
};

type Shared<T> = Arc<SpinNoIrqLock<T>>;

/// User task, a.k.a. process control block
pub struct Task {
    pub pid: PidHandle,
    /// command
    pub comm: String,
    /// Whether this process is a zombie process
    pub state: TaskState,
    /// The process's address space
    pub memory_space: Shared<MemorySpace>,
    /// Parent process
    pub parent: Option<Weak<Task>>,
    /// Children processes
    pub children: Vec<Arc<Task>>,
    /// Exit code of the current process
    pub exit_code: AtomicI8,
    pub trap_context: SyncUnsafeCell<TrapContext>,
    pub waker: Option<Waker>,
    pub ustack_top: usize,
}

pub enum TaskState {
    Running,
    Zombie,
}

impl Task {
    pub fn new() -> Self {}
    pub fn pid(&self) -> usize {
        stack_trace!();
        self.pid.0
    }

    pub fn exit_code(&self) -> i8 {
        stack_trace!();
        self.exit_code
    }

    /// Get the mutable ref of trap context
    pub fn trap_context_mut(&mut self) -> &mut TrapContext {
        stack_trace!();
        unsafe { &mut *self.trap_context.get() }
    }

    /// Set waker for this thread
    pub fn set_waker(&self, waker: Waker) {
        stack_trace!();
        unsafe {
            *self.waker = Some(waker);
        }
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        stack_trace!();
        log::info!("task {} died!", self.pid());
    }
}
