use alloc::{
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    cell::SyncUnsafeCell,
    sync::atomic::{AtomicI8, Ordering},
    task::Waker,
};

use sync::mutex::SpinNoIrqLock;

use super::pid::PidHandle;
use crate::{
    mm::MemorySpace,
    stack_trace,
    task::{
        manager::TASK_MANAGER,
        pid::{self, alloc_pid},
        schedule,
    },
    trap::TrapContext,
};

type Shared<T> = Arc<SpinNoIrqLock<T>>;

/// User task, a.k.a. process control block
pub struct Task {
    pub pid: PidHandle,
    // /// command
    // pub comm: String,
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
    pub waker: SyncUnsafeCell<Option<Waker>>,
    pub ustack_top: usize,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum TaskState {
    Running,
    Zombie,
}

impl Task {
    pub fn new(elf_data: &[u8]) {
        stack_trace!();
        let (memory_space, user_sp_top, entry_point, _auxv) = MemorySpace::from_elf(elf_data);

        let trap_context = TrapContext::app_init_context(entry_point, user_sp_top);
        // Alloc a pid
        let pid = alloc_pid();
        let task = Arc::new(Self {
            pid,
            state: TaskState::Running,
            parent: None,
            children: Vec::new(),
            exit_code: AtomicI8::new(0),
            trap_context: SyncUnsafeCell::new(trap_context),
            memory_space: Arc::new(SpinNoIrqLock::new(memory_space)),
            waker: SyncUnsafeCell::new(None),
            ustack_top: user_sp_top,
        });

        TASK_MANAGER.add_task(task.pid(), &task);
        log::debug!("create a new process, pid {}", task.pid());
        schedule::spawn_user_task(task);
    }
    pub fn pid(&self) -> usize {
        stack_trace!();
        self.pid.0
    }

    pub fn exit_code(&self) -> i8 {
        stack_trace!();
        self.exit_code.load(Ordering::Relaxed)
    }

    /// Get the mutable ref of trap context
    pub fn trap_context_mut(&self) -> &mut TrapContext {
        stack_trace!();
        unsafe { &mut *self.trap_context.get() }
    }

    /// Set waker for this thread
    pub fn set_waker(&self, waker: Waker) {
        stack_trace!();
        unsafe {
            (*self.waker.get()) = Some(waker);
        }
    }

    pub fn is_zombie(&self) -> bool {
        self.state == TaskState::Zombie
    }

    pub fn activate(&self) {
        self.memory_space.lock().activate()
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        stack_trace!();
        log::info!("task {} died!", self.pid());
    }
}
