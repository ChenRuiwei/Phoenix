use alloc::{
    collections::BTreeMap,
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    cell::SyncUnsafeCell,
    sync::atomic::{AtomicI32, AtomicUsize, Ordering},
    task::Waker,
};

use arch::memory::sfence_vma_all;
use config::{mm::USER_STACK_SIZE, process::INIT_PROC_PID};
use futex::Futexes;
use memory::VirtAddr;
use signal::{
    action::{SigHandlers, SigPending},
    siginfo::{SigDetails, SigInfo},
    signal_stack::SignalStack,
    sigset::{Sig, SigSet},
};
use sync::mutex::SpinNoIrqLock;
use time::stat::TaskTimeStat;
use vfs::{fd_table::FdTable, sys_root_dentry};
use vfs_core::Dentry;

use super::{
    resource::CpuMask,
    signal::ITimer,
    tid::{Pid, Tid, TidHandle},
};
use crate::{
    generate_accessors, generate_atomic_accessors, generate_state_methods, generate_with_methods,
    ipc::shm::SHARED_MEMORY_MANAGER,
    mm::{memory_space::init_stack, MemorySpace, UserWritePtr},
    processor::env::within_sum,
    syscall,
    task::{
        manager::TASK_MANAGER,
        schedule,
        tid::{alloc_tid, TidAddress},
    },
    trap::TrapContext,
};

type Shared<T> = Arc<SpinNoIrqLock<T>>;

fn new_shared<T>(data: T) -> Shared<T> {
    Arc::new(SpinNoIrqLock::new(data))
}

/// User task control block, a.k.a. process control block.
///
/// We treat processes and threads as tasks, consistent with the approach
/// adopted by Linux. A process is a task that is the leader of a `ThreadGroup`.
pub struct Task {
    // Immutable
    /// Task identifier handle.
    tid: TidHandle,
    /// Weak reference to the leader task. `None` if this task is the leader.
    leader: Option<Weak<Task>>,
    /// Indicates if the task is the leader of its thread group.
    is_leader: bool,

    // Mutable
    /// Indicates if the task is a zombie. Protected by a spin lock due to
    /// potential access by other tasks.
    state: SpinNoIrqLock<TaskState>,
    /// The address space of the process.
    memory_space: Shared<MemorySpace>,
    /// Map of start address of shared memory areas to their keys in the shared
    /// memory manager.
    shm_ids: Shared<BTreeMap<VirtAddr, usize>>,
    /// Parent process
    parent: Shared<Option<Weak<Task>>>,
    /// Children processes
    // NOTE: Arc<Task> can only be hold by `Hart`, `UserTaskFuture` and parent `Task`. Unused task
    // will be automatically dropped by previous two structs. However, it should be treated with
    // great care to drop task in `children`.
    children: Shared<BTreeMap<Tid, Arc<Task>>>,
    /// Exit code of the current process
    exit_code: AtomicI32,
    /// Trap context for the task.
    trap_context: SyncUnsafeCell<TrapContext>,
    /// Waker to add the task back to the scheduler.
    waker: SyncUnsafeCell<Option<Waker>>,
    /// Thread group containing this task.
    thread_group: Shared<ThreadGroup>,
    /// File descriptor table.
    fd_table: Shared<FdTable>,
    /// Current working directory dentry.
    cwd: Shared<Arc<dyn Dentry>>,
    /// Pending signals for the task.
    sig_pending: SpinNoIrqLock<SigPending>,
    /// Signal handlers
    sig_handlers: Shared<SigHandlers>,
    /// Optional signal stack for the task, settable via `sys_signalstack`.
    sig_mask: SyncUnsafeCell<SigSet>,
    /// Optional signal stack for the task, settable via `sys_signalstack`.
    sig_stack: SyncUnsafeCell<Option<SignalStack>>,
    /// Pointer to the user context for signal handling.
    sig_ucontext_ptr: AtomicUsize,
    /// Statistics for task execution times.
    time_stat: SyncUnsafeCell<TaskTimeStat>,
    /// Interval timers for the task.
    itimers: Shared<[ITimer; 3]>,
    /// Futexes used by the task.
    futexes: Shared<Futexes>,
    ///
    tid_address: SyncUnsafeCell<TidAddress>,
    cpus_allowed: SyncUnsafeCell<CpuMask>,
}

impl core::fmt::Debug for Task {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Task").field("tid", &self.tid()).finish()
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        log::info!("task {} died!", self.tid());
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TaskState {
    /// The task is currently running or ready to run, occupying the CPU and
    /// executing its code.
    Running,
    /// The task has terminated, but its process control block (PCB) still
    /// exists for the parent process to read its exit status.
    Zombie,
    /// The task has been stopped, usually due to receiving a stop signal (e.g.,
    /// SIGSTOP). It can be resumed with a continue signal (e.g., SIGCONT).
    Stopped,
    /// The task is waiting for an event, such as the completion of an I/O
    /// operation or the release of a resource. In this state, the task can
    /// be interrupted by signals. If a signal is sent to the task, it will be
    /// awakened from sleep to handle the signal. This state allows for more
    /// flexible scheduling since the task can respond to signals.
    Interruptable,
    /// The task is also waiting for an event, but it cannot be interrupted by
    /// signals in this state. Even if a signal is sent to the task, it will
    /// not respond immediately and will only be awakened when the awaited event
    /// occurs. This state is typically used to ensure that critical
    /// operations are not interrupted, maintaining data consistency and
    /// operation atomicity.
    UnInterruptable,
}

impl Task {
    // you can use is_running() / set_running()、 is_zombie() / set_zombie()
    generate_state_methods!(Running, Zombie, Stopped, Interruptable, UnInterruptable);
    generate_accessors!(waker: Option<Waker>, tid_address: TidAddress, sig_mask: SigSet, sig_stack: Option<SignalStack>, time_stat: TaskTimeStat, cpus_allowed: CpuMask);
    generate_atomic_accessors!(exit_code: i32, sig_ucontext_ptr: usize);
    generate_with_methods!(
        fd_table: FdTable,
        children: BTreeMap<Tid, Arc<Task>>,
        memory_space: MemorySpace,
        thread_group: ThreadGroup,
        sig_pending: SigPending,
        itimers: [ITimer; 3],
        futexes: Futexes,
        sig_handlers: SigHandlers,
        state: TaskState,
        shm_ids: BTreeMap<VirtAddr, usize>
    );
    // TODO: this function is not clear, may be replaced with exec
    pub fn spawn_from_elf(elf_data: &[u8]) {
        let (memory_space, user_sp_top, entry_point, _auxv) = MemorySpace::from_elf(elf_data);
        let trap_context = TrapContext::new(entry_point, user_sp_top);
        let task = Arc::new(Self {
            tid: alloc_tid(),
            leader: None,
            is_leader: true,
            state: SpinNoIrqLock::new(TaskState::Running),
            parent: new_shared(None),
            children: new_shared(BTreeMap::new()),
            exit_code: AtomicI32::new(0),
            trap_context: SyncUnsafeCell::new(trap_context),
            memory_space: new_shared(memory_space),
            waker: SyncUnsafeCell::new(None),
            thread_group: new_shared(ThreadGroup::new()),
            fd_table: new_shared(FdTable::new()),
            cwd: new_shared(sys_root_dentry()),
            sig_pending: SpinNoIrqLock::new(SigPending::new()),
            sig_mask: SyncUnsafeCell::new(SigSet::empty()),
            sig_handlers: new_shared(SigHandlers::new()),
            sig_stack: SyncUnsafeCell::new(None),
            time_stat: SyncUnsafeCell::new(TaskTimeStat::new()),
            sig_ucontext_ptr: AtomicUsize::new(0),
            itimers: new_shared([
                ITimer::new_real(),
                ITimer::new_virtual(),
                ITimer::new_prof(),
            ]),
            futexes: new_shared(Futexes::new()),
            tid_address: SyncUnsafeCell::new(TidAddress::new()),
            cpus_allowed: SyncUnsafeCell::new(CpuMask::CPU_ALL),
            shm_ids: new_shared(BTreeMap::new()),
        });
        task.thread_group.lock().push(task.clone());

        TASK_MANAGER.add(&task);
        log::debug!("create a new process, pid {}", task.tid());
        schedule::spawn_user_task(task);
    }

    pub fn parent(&self) -> Option<Weak<Self>> {
        self.parent.lock().clone()
    }

    pub fn children(&self) -> BTreeMap<Tid, Arc<Self>> {
        self.children.lock().clone()
    }

    pub fn state(&self) -> TaskState {
        *self.state.lock()
    }

    pub fn add_child(&self, child: Arc<Task>) {
        log::debug!("[Task::add_child] add a new child tid {}", child.tid());
        self.children
            .lock()
            .try_insert(child.tid(), child)
            .expect("try add child with a duplicate tid");
    }

    pub fn remove_child(&self, tid: Tid) {
        self.children.lock().remove(&tid);
    }

    /// the task is a process or a thread
    pub fn is_leader(&self) -> bool {
        self.is_leader
    }

    pub fn leader(self: &Arc<Self>) -> Arc<Self> {
        if self.is_leader() {
            self.clone()
        } else {
            self.leader.as_ref().cloned().unwrap().upgrade().unwrap()
        }
    }

    /// Pid means tgid.
    pub fn pid(self: &Arc<Self>) -> Pid {
        self.leader().tid()
    }

    pub fn tid(&self) -> Tid {
        self.tid.0
    }

    pub fn ppid(&self) -> Pid {
        self.parent()
            .expect("Call ppid without a parent")
            .upgrade()
            .unwrap()
            .pid()
    }

    /// Get the mutable ref of `TrapContext`.
    pub fn trap_context_mut(&self) -> &mut TrapContext {
        unsafe { &mut *self.trap_context.get() }
    }

    pub fn wake(&self) {
        debug_assert!(!(self.is_running() || self.is_zombie()));
        let waker = self.waker_ref();
        waker.as_ref().unwrap().wake_by_ref();
    }

    pub fn cwd(&self) -> Arc<dyn Dentry> {
        self.cwd.lock().clone()
    }

    pub fn set_cwd(&self, dentry: Arc<dyn Dentry>) {
        *self.cwd.lock() = dentry;
    }

    pub unsafe fn switch_page_table(&self) {
        self.memory_space.lock().switch_page_table()
    }

    // TODO:
    pub fn do_clone(
        self: &Arc<Self>,
        flags: syscall::CloneFlags,
        stack: Option<VirtAddr>,
        chilren_tid_ptr: usize,
    ) -> Arc<Self> {
        use syscall::CloneFlags;
        let tid = alloc_tid();
        let mut trap_context = SyncUnsafeCell::new(*self.trap_context_mut());
        let state = SpinNoIrqLock::new(self.state());

        let leader;
        let is_leader;
        let parent;
        let children;
        let thread_group;
        let cwd;
        let itimers;
        let futexes;
        let sig_handlers = if flags.contains(CloneFlags::SIGHAND) {
            self.sig_handlers.clone()
        } else {
            new_shared(self.with_sig_handlers(|handlers| handlers.clone()))
        };
        if flags.contains(CloneFlags::THREAD) {
            is_leader = false;
            leader = Some(Arc::downgrade(self));
            parent = self.parent.clone();
            children = self.children.clone();
            thread_group = self.thread_group.clone();
            itimers = self.itimers.clone();
            cwd = self.cwd.clone();
            futexes = self.futexes.clone();
        } else {
            is_leader = true;
            leader = None;
            parent = new_shared(Some(Arc::downgrade(self)));
            children = new_shared(BTreeMap::new());
            thread_group = new_shared(ThreadGroup::new());
            itimers = new_shared([
                ITimer::new_real(),
                ITimer::new_virtual(),
                ITimer::new_prof(),
            ]);
            cwd = new_shared(self.cwd());
            futexes = new_shared(Futexes::new());
        }

        let memory_space;
        if flags.contains(CloneFlags::VM) {
            memory_space = self.memory_space.clone();
        } else {
            memory_space =
                new_shared(self.with_mut_memory_space(|m| MemorySpace::from_user_lazily(m)));
            // TODO: avoid flushing global entries like kernel mappings
            unsafe { sfence_vma_all() };
        }

        let fd_table = if flags.contains(CloneFlags::FILES) {
            self.fd_table.clone()
        } else {
            new_shared(self.fd_table.lock().clone())
        };

        if let Some(sp) = stack {
            trap_context.get_mut().set_user_sp(sp.bits());
        }
        let tid_address = if flags.contains(CloneFlags::CHILD_CLEARTID) {
            log::warn!("CloneFlags::CHILD_CLEARTID");
            SyncUnsafeCell::new(TidAddress {
                set_child_tid: None,
                clear_child_tid: Some(chilren_tid_ptr),
            })
        } else {
            SyncUnsafeCell::new(TidAddress::new())
        };

        let new = Arc::new(Self {
            tid,
            leader,
            is_leader,
            cwd,
            state,
            parent,
            children,
            exit_code: AtomicI32::new(0),
            trap_context,
            memory_space,
            waker: SyncUnsafeCell::new(None),
            thread_group,
            fd_table,
            sig_pending: SpinNoIrqLock::new(SigPending::new()),
            // A child created via fork(2) inherits a copy of its parent's signal mask;
            sig_mask: SyncUnsafeCell::new(self.sig_mask_ref().clone()),
            sig_handlers,
            sig_stack: SyncUnsafeCell::new(None),
            time_stat: SyncUnsafeCell::new(TaskTimeStat::new()),
            sig_ucontext_ptr: AtomicUsize::new(0),
            itimers,
            futexes,
            tid_address,
            cpus_allowed: SyncUnsafeCell::new(CpuMask::CPU_ALL),
            // After a fork(2), the child inherits the attached shared memory segments.
            shm_ids: self.shm_ids.clone(),
        });

        if !flags.contains(CloneFlags::THREAD) {
            self.add_child(new.clone());
        }
        new.with_mut_thread_group(|tg| tg.push(new.clone()));

        if flags.contains(CloneFlags::CHILD_SETTID) {
            log::warn!("CloneFlags::CHILD_SETTID");
            UserWritePtr::from_usize(chilren_tid_ptr)
                .write(self, new.tid())
                .expect("CloneFlags::CHILD_SETTID error");
        }

        TASK_MANAGER.add(&new);
        new
    }

    // TODO: figure out what should be reserved across this syscall
    // TODO: support CLOSE_ON_EXEC flag may be
    pub fn do_execve(&self, elf_data: &[u8], argv: Vec<String>, envp: Vec<String>) {
        log::debug!("[Task::do_execve] parsing elf");
        let mut memory_space = MemorySpace::new_user();
        let (entry, auxv) = memory_space.parse_and_map_elf(elf_data);

        // NOTE: should do termination before switching page table, so that other
        // threads will trap in by page fault and be handled by `do_exit`
        log::debug!("[Task::do_execve] terminating all threads except the leader");
        let pid = self.with_thread_group(|tg| {
            let mut pid = 0;
            for t in tg.iter() {
                if !t.is_leader() {
                    t.set_zombie();
                } else {
                    pid = t.tid.0;
                }
            }
            pid
        });

        log::debug!("[Task::do_execve] changing memory space");
        // NOTE: need to switch to new page table first before dropping old page table,
        // otherwise, there will be a vacuum period without page table which will cause
        // random errors in smp situation
        unsafe { memory_space.switch_page_table() };
        self.with_mut_memory_space(|m| *m = memory_space);

        // alloc stack, and push argv, envp and auxv
        log::debug!("[Task::do_execve] allocing stack");
        let sp_init = self.with_mut_memory_space(|m| m.alloc_stack(USER_STACK_SIZE));

        let (sp, argc, argv, envp) = within_sum(|| init_stack(sp_init, argv, envp, auxv));

        // alloc heap
        self.with_mut_memory_space(|m| m.alloc_heap_lazily());

        // close fd on exec
        self.with_mut_fd_table(|table| table.do_close_on_exec());

        // init trap context
        self.trap_context_mut()
            .init_user(sp, entry, argc, argv, envp);

        // Any alternate signal stack is not preserved
        *self.sig_stack() = None;

        // During an execve, the dispositions of handled signals are reset
        // to the default; the dispositions of ignored signals are left unchanged
        self.with_mut_sig_handlers(|handlers| handlers.reset_user_defined());

        // After an execve(2), all attached shared memory segments are detached from the
        // process.
        self.with_mut_shm_ids(|ids| {
            for (_, shm_id) in ids.iter() {
                SHARED_MEMORY_MANAGER.detach(*shm_id, pid);
            }
            *ids = BTreeMap::new();
        });
    }

    // NOTE: After all of the threads in a thread group is terminated, the parent
    // process of the thread group is sent a SIGCHLD (or other termination) signal.
    // WARN: do not call this function directly if a task should be terminated,
    // instead, call `set_zombie`
    // TODO:
    pub fn do_exit(self: &Arc<Self>) {
        log::info!("thread {} do exit", self.tid());
        assert_ne!(
            self.tid(),
            INIT_PROC_PID,
            "initproc die!!!, sepc {:#x}",
            self.trap_context_mut().sepc
        );

        log::debug!("[Task::do_exit] set children to be zombie and reparent them to init");
        debug_assert_ne!(self.tid(), INIT_PROC_PID);
        self.with_mut_children(|children| {
            if children.is_empty() {
                return;
            }
            let init_proc = TASK_MANAGER.init_proc();
            for c in children.values() {
                c.set_zombie();
                *c.parent.lock() = Some(Arc::downgrade(&init_proc));
            }
            init_proc.children.lock().extend(children.clone());
        });

        if let Some(address) = self.tid_address_ref().clear_child_tid {
            log::info!("[do_exit] clear_child_tid: {}", address);
            UserWritePtr::from(address)
                .write(self, 0)
                .expect("tid address write error");
            self.with_mut_futexes(|futexes| futexes.wake(address as u32, 1));
        }

        // NOTE: leader will be removed by parent calling `sys_wait4`
        if !self.is_leader() {
            self.with_mut_thread_group(|tg| tg.remove(self));
            TASK_MANAGER.remove(self.tid())
        } else {
            // TODO: drop most of resource
            if let Some(parent) = self.parent() {
                let parent = parent.upgrade().unwrap();
                parent.receive_siginfo(
                    SigInfo {
                        sig: Sig::SIGCHLD,
                        code: SigInfo::CLD_EXITED,
                        details: SigDetails::CHLD {
                            pid: self.pid(),
                            status: self.exit_code(),
                            utime: self.time_stat().user_time(),
                            stime: self.time_stat().sys_time(),
                        },
                    },
                    false,
                );
            }
            // Upon _exit(2), all attached shared memory segments are detached from the
            // process.
            self.with_shm_ids(|ids| {
                for (_, shm_id) in ids.iter() {
                    SHARED_MEMORY_MANAGER.detach(*shm_id, self.pid());
                }
            });
        }
    }
}

/// Hold a group of threads which belongs to the same process.
pub struct ThreadGroup {
    members: BTreeMap<Tid, Weak<Task>>,
}

impl ThreadGroup {
    pub fn new() -> Self {
        Self {
            members: BTreeMap::new(),
        }
    }

    pub fn push(&mut self, task: Arc<Task>) {
        self.members.insert(task.tid(), Arc::downgrade(&task));
    }

    pub fn remove(&mut self, task: &Task) {
        self.members.remove(&task.tid());
    }

    pub fn iter(&self) -> impl Iterator<Item = Arc<Task>> + '_ {
        self.members.values().map(|t| t.upgrade().unwrap())
    }
}
