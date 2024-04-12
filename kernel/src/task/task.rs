use alloc::{
    collections::BTreeMap,
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    cell::SyncUnsafeCell,
    sync::atomic::{AtomicI32, AtomicI8, Ordering},
    task::Waker,
};

use signal::{signal_stack::SignalStack, Signal};
use sync::mutex::SpinNoIrqLock;

use super::pid::{Pid, PidHandle};
use crate::{
    mm::MemorySpace,
    task::{
        manager::TASK_MANAGER,
        pid::{self, alloc_pid},
        schedule,
    },
    trap::TrapContext,
};

type Shared<T> = Arc<SpinNoIrqLock<T>>;

fn new_shared<T>(data: T) -> Shared<T> {
    Arc::new(SpinNoIrqLock::new(data))
}

/// User task control block, a.k.a. process control block
///
/// We treat processes and threads as tasks, consistent with the approach
/// adopted by Linux.
pub struct Task {
    ///
    pid: PidHandle,
    /// Whether this process is a zombie process
    pub state: SpinNoIrqLock<TaskState>,
    /// The process's address space
    pub memory_space: Shared<MemorySpace>,
    /// Parent process
    pub parent: Shared<Option<Weak<Task>>>,
    /// Children processes
    pub children: Shared<Vec<Arc<Task>>>,
    /// Exit code of the current process
    pub exit_code: AtomicI32,
    ///
    pub trap_context: SyncUnsafeCell<TrapContext>,
    ///
    pub waker: SyncUnsafeCell<Option<Waker>>,
    ///
    pub ustack_top: usize,
    ///
    pub thread_group: Shared<ThreadGroup>,
    pub signal: SpinNoIrqLock<Signal>,
    /// user can define sig_stack by sys_signalstack
    pub sig_stack: SyncUnsafeCell<Option<SignalStack>>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TaskState {
    Running,
    Zombie,
}

macro_rules! with_ {
    ($name:ident, $ty:ty) => {
        paste::paste! {
            pub fn [<with_ $name>]<T>(&self, f: impl FnOnce(&$ty) -> T) -> T {
                f(& self.$name.lock())
            }
            pub fn [<with_mut_ $name>]<T>(&self, f: impl FnOnce(&mut $ty) -> T) -> T {
                f(&mut self.$name.lock())
            }
        }
    };
}

impl Task {
    // TODO: this function is not clear, may be replaced with exec
    pub fn from_elf(elf_data: &[u8]) {
        let (memory_space, user_sp_top, entry_point, _auxv) = MemorySpace::from_elf(elf_data);

        let trap_context = TrapContext::new(entry_point, user_sp_top);
        let task = Arc::new(Self {
            pid: alloc_pid(),
            state: SpinNoIrqLock::new(TaskState::Running),
            parent: new_shared(None),
            children: new_shared(Vec::new()),
            exit_code: AtomicI32::new(0),
            trap_context: SyncUnsafeCell::new(trap_context),
            memory_space: new_shared(memory_space),
            waker: SyncUnsafeCell::new(None),
            ustack_top: user_sp_top,
            thread_group: new_shared(ThreadGroup::new()),
            signal: SpinNoIrqLock::new(Signal::new()),
            sig_stack: SyncUnsafeCell::new(None),
        });

        task.thread_group.lock().push_leader(task.clone());

        TASK_MANAGER.add_task(task.pid(), &task);
        log::debug!("create a new process, pid {}", task.pid());
        schedule::spawn_user_task(task);
    }

    fn parent(&self) -> Option<Weak<Self>> {
        self.parent.lock().clone()
    }

    pub fn pid(&self) -> Pid {
        self.pid.0
    }

    pub fn ppid(&self) -> Pid {
        self.parent()
            .expect("Call ppid without a parent")
            .upgrade()
            .unwrap()
            .pid()
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code.load(Ordering::Relaxed)
    }

    pub fn set_exit_code(&self, exit_code: i32) {
        self.exit_code.store(exit_code, Ordering::Relaxed);
    }

    /// Get the mutable ref of `TrapContext`.
    pub fn trap_context_mut(&self) -> &mut TrapContext {
        unsafe { &mut *self.trap_context.get() }
    }

    /// Set waker for this thread
    pub fn set_waker(&self, waker: Waker) {
        unsafe {
            (*self.waker.get()) = Some(waker);
        }
    }

    pub fn set_zombie(&self) {
        *self.state.lock() = TaskState::Zombie
    }

    pub fn is_zombie(&self) -> bool {
        *self.state.lock() == TaskState::Zombie
    }

    pub fn get_signal_stack(&self) -> &mut Option<SignalStack> {
        unsafe { &mut *self.sig_stack.get() }
    }

    pub fn set_signal_stack(&self, stack: Option<SignalStack>) {
        unsafe {
            *self.sig_stack.get() = stack;
        }
    }

    pub unsafe fn switch_page_table(&self) {
        self.memory_space.lock().switch_page_table()
    }

    // TODO:
    pub fn do_clone(&self) {}

    pub fn do_execve(&self, data: &[u8], argv: Vec<String>, envp: Vec<String>) {}

    // TODO:
    pub fn do_exit(&self) {
        // Send SIGCHLD to parent
        if let Some(parent) = self.parent() {
            let parent = parent.upgrade().unwrap();
        }

        // Reparent children

        // Release all fd
    }

    with_!(memory_space, MemorySpace);
}

impl Drop for Task {
    fn drop(&mut self) {
        log::info!("task {} died!", self.pid());
    }
}

/// Hold a group of threads which belongs to the same process
pub struct ThreadGroup {
    members: BTreeMap<Pid, Arc<Task>>,
    leader: Option<Weak<Task>>,
}

impl ThreadGroup {
    pub fn new() -> Self {
        Self {
            members: BTreeMap::new(),
            leader: None,
        }
    }

    pub fn push_leader(&mut self, leader: Arc<Task>) {
        debug_assert!(self.leader.is_none());
        debug_assert!(self.members.is_empty());

        self.leader = Some(Arc::downgrade(&leader));
        self.members.insert(leader.pid(), leader);
    }

    pub fn push(&mut self, task: Arc<Task>) {
        debug_assert!(self.leader.is_some());
        self.members.insert(task.pid(), task);
    }

    pub fn remove(&mut self, thread: &Task) {
        debug_assert!(self.leader.is_some());
        self.members.remove(&thread.pid());
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn tgid(&self) -> Pid {
        self.leader.as_ref().unwrap().upgrade().unwrap().pid()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<Task>> {
        self.members.values()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Arc<Task>> {
        self.members.values_mut()
    }
}
