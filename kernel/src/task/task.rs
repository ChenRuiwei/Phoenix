use alloc::{
    collections::BTreeMap,
    string::String,
    sync::{Arc, Weak},
    task,
    vec::Vec,
};
use core::{
    cell::SyncUnsafeCell,
    sync::atomic::{AtomicI32, AtomicI8, Ordering},
    task::Waker,
};

use arch::{memory::sfence_vma_all, time::get_time_duration};
use config::{mm::USER_STACK_SIZE, process::INITPROC_PID};
use memory::VirtAddr;
use signal::{signal_stack::SignalStack, Signal};
use sync::mutex::SpinNoIrqLock;
use time::stat::TaskTimeStat;
use virtio_drivers::PAGE_SIZE;

use super::tid::{Pid, Tid, TidHandle};
use crate::{
    mm::{
        memory_space::{
            self,
            vm_area::{MapPerm, VmArea},
        },
        MemorySpace,
    },
    syscall,
    task::{
        aux::{generate_early_auxv, AuxHeader, AT_BASE, AT_PHDR},
        manager::TASK_MANAGER,
        schedule,
        tid::alloc_tid,
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
/// adopted by Linux. A process is a task that is the leader of a `ThreadGroup`.
pub struct Task {
    // Immutable
    /// Tid of the task.
    tid: TidHandle,
    /// Whether the task is the leader.
    is_leader: bool,

    // Mutable
    /// Whether this task is a zombie. Locked because of other task may operate
    /// this state, e.g. execve will kill other tasks.
    pub state: SpinNoIrqLock<TaskState>,
    /// The process's address space
    pub memory_space: Shared<MemorySpace>,
    /// Parent process
    pub parent: Shared<Option<Weak<Task>>>,
    /// Children processes
    // NOTE: Arc<Task> can only be hold by `Hart`, `UserTaskFuture` and parent `Task`. Unused task
    // will be automatically dropped by previous two structs. However, it should be treated with
    // great care to drop task in `children`.
    pub children: Shared<BTreeMap<Tid, Arc<Task>>>,
    /// Exit code of the current process
    pub exit_code: AtomicI32,
    ///
    pub trap_context: SyncUnsafeCell<TrapContext>,
    ///
    pub waker: SyncUnsafeCell<Option<Waker>>,
    ///
    pub thread_group: Shared<ThreadGroup>,
    ///
    pub signal: SpinNoIrqLock<Signal>,
    /// User can set `sig_stack` by `sys_signalstack`.
    pub sig_stack: SyncUnsafeCell<Option<SignalStack>>,

    pub time_stat: SyncUnsafeCell<TaskTimeStat>,
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
    pub fn spawn_from_elf(elf_data: &[u8]) {
        let (memory_space, user_sp_top, entry_point, _auxv) = MemorySpace::from_elf(elf_data);

        let trap_context = TrapContext::new(entry_point, user_sp_top);
        let task = Arc::new(Self {
            tid: alloc_tid(),
            is_leader: true,
            state: SpinNoIrqLock::new(TaskState::Running),
            parent: new_shared(None),
            children: new_shared(BTreeMap::new()),
            exit_code: AtomicI32::new(0),
            trap_context: SyncUnsafeCell::new(trap_context),
            memory_space: new_shared(memory_space),
            waker: SyncUnsafeCell::new(None),
            thread_group: new_shared(ThreadGroup::new()),
            signal: SpinNoIrqLock::new(Signal::new()),
            sig_stack: SyncUnsafeCell::new(None),
            time_stat: SyncUnsafeCell::new(TaskTimeStat::new()),
        });

        task.thread_group.lock().push_leader(task.clone());

        TASK_MANAGER.add(&task);
        log::debug!("create a new process, pid {}", task.tid());
        schedule::spawn_user_task(task);
    }

    fn parent(&self) -> Option<Weak<Self>> {
        self.parent.lock().clone()
    }

    pub fn children(&self) -> BTreeMap<Tid, Arc<Self>> {
        self.children.lock().clone()
    }

    fn state(&self) -> TaskState {
        *self.state.lock()
    }

    pub fn add_child(&self, child: Arc<Task>) {
        self.children
            .lock()
            .try_insert(child.tid(), child)
            .expect("try add child with a duplicate tid");
    }

    pub fn remove_child(&self, tid: Tid) {
        self.children.lock().remove(&tid);
    }

    pub fn is_leader(&self) -> bool {
        self.is_leader
    }

    /// Pid means tgid.
    pub fn pid(&self) -> Pid {
        self.thread_group.lock().tgid()
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

    pub fn signal_stack(&self) -> &mut Option<SignalStack> {
        unsafe { &mut *self.sig_stack.get() }
    }

    pub fn set_signal_stack(&self, stack: Option<SignalStack>) {
        unsafe {
            *self.sig_stack.get() = stack;
        }
    }

    pub fn time_stat(&self) -> &mut TaskTimeStat {
        unsafe { &mut *self.time_stat.get() }
    }

    pub unsafe fn switch_page_table(&self) {
        self.memory_space.lock().switch_page_table()
    }

    pub fn map_all_threads(&self, mut f: impl FnMut(&Self)) {
        self.with_mut_thread_group(|tg| {
            for t in tg.iter() {
                f(&t)
            }
        });
    }

    pub fn parse_and_map_elf(&self, elf_data: &[u8]) -> (usize, Vec<AuxHeader>) {
        const ELF_MAGIC: [u8; 4] = [0x7f, 0x45, 0x4c, 0x46];

        // map program headers of elf, with U flag
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        assert_eq!(elf_header.pt1.magic, ELF_MAGIC, "invalid elf!");
        let entry = elf_header.pt2.entry_point() as usize;
        let ph_entry_size = elf_header.pt2.ph_entry_size() as usize;
        let ph_count = elf_header.pt2.ph_count() as usize;

        let mut auxv = generate_early_auxv(ph_entry_size, ph_count, entry);

        auxv.push(AuxHeader::new(AT_BASE, 0));

        let (max_end_vpn, header_va) = self.with_mut_memory_space(|m| m.map_elf(&elf, 0.into()));

        let ph_head_addr = header_va.0 + elf.header.pt2.ph_offset() as usize;
        log::debug!(
            "[parse_and_map_elf] AT_PHDR  ph_head_addr is {:x} ",
            ph_head_addr
        );
        auxv.push(AuxHeader::new(AT_PHDR, ph_head_addr));

        (entry, auxv)
    }

    // TODO:
    pub fn do_clone(
        self: &Arc<Self>,
        flags: syscall::CloneFlags,
        user_stack_begin: Option<VirtAddr>,
    ) -> Arc<Self> {
        use syscall::CloneFlags;
        let tid = alloc_tid();

        let trap_context = SyncUnsafeCell::new(*self.trap_context_mut());
        let state = SpinNoIrqLock::new(self.state());
        let exit_code = AtomicI32::new(self.exit_code());

        let is_leader;
        let parent;
        let children;
        let thread_group;

        if flags.contains(CloneFlags::THREAD) {
            is_leader = false;
            parent = self.parent.clone();
            children = self.children.clone();
            // will add the new task into the group later
            thread_group = self.thread_group.clone();
        } else {
            is_leader = true;
            parent = new_shared(Some(Arc::downgrade(self)));
            children = new_shared(BTreeMap::new());
            thread_group = new_shared(ThreadGroup::new());
        }

        let memory_space;
        if flags.contains(CloneFlags::VM) {
            memory_space = self.memory_space.clone();
        } else {
            debug_assert!(user_stack_begin.is_none());
            memory_space =
                new_shared(self.with_mut_memory_space(|m| MemorySpace::from_user_lazily(m)));
            // TODO: avoid flushing global entries like kernel mappings
            unsafe { sfence_vma_all() };
        }

        let new = Arc::new(Self {
            tid,
            is_leader,
            state,
            parent,
            children,
            exit_code: AtomicI32::new(0),
            trap_context,
            memory_space,
            waker: SyncUnsafeCell::new(None),
            thread_group,
            signal: SpinNoIrqLock::new(Signal::new()),
            sig_stack: SyncUnsafeCell::new(None),
            time_stat: SyncUnsafeCell::new(TaskTimeStat::new()),
        });

        if flags.contains(CloneFlags::THREAD) {
            new.with_mut_thread_group(|tg| tg.push(new.clone()));
        } else {
            new.with_mut_thread_group(|g| g.push_leader(new.clone()));
            self.add_child(new.clone());
        }

        TASK_MANAGER.add(&new);
        new
    }

    // TODO:
    pub fn do_execve(&self, elf_data: &[u8], argv: Vec<String>, envp: Vec<String>) {
        log::debug!("[Task::do_execve] parsing elf");
        let mut memory_space = MemorySpace::new_user();
        let (entry, auxv) = memory_space.parse_and_map_elf(elf_data);

        // NOTE: should do termination before switching page table, so that other
        // threads will trap in by page fault but be terminated before handling
        log::debug!("[Task::do_execve] terminating all threads except the leader");
        self.with_thread_group(|tg| {
            for t in tg.iter() {
                if !t.is_leader() {
                    t.set_zombie();
                }
            }
        });

        log::debug!("[Task::do_execve] changing memory space");
        // NOTE: need to switch to new page table first before dropping old page table,
        // otherwise, there will be a vacuum period without page table which will cause
        // random errors in smp situation
        unsafe { memory_space.switch_page_table() };
        self.with_mut_memory_space(|m| *m = memory_space);

        // alloc stack, and push argv, envp and auxv
        log::debug!("[Task::do_execve] allocing stack");
        let stack_begin = self.with_mut_memory_space(|m| m.alloc_stack(USER_STACK_SIZE));

        // alloc heap
        self.with_mut_memory_space(|m| m.alloc_heap_lazily());

        // init trap context
        self.trap_context_mut()
            .init_user(stack_begin.into(), entry, 0, 0, 0);
    }

    // NOTE: After all of the threads in a thread group is terminated, the parent
    // process of the thread group is sent a SIGCHLD (or other termination) signal.
    // TODO:
    pub fn do_exit(self: &Arc<Self>) {
        log::info!("thread {} do exit", self.tid());
        if self.tid() == INITPROC_PID {
            panic!("initproc die!!!, sepc {:#x}", self.trap_context_mut().sepc);
        }

        // TODO: send SIGCHLD to parent if this is the leader
        if self.is_leader() {
            if let Some(parent) = self.parent() {
                let parent = parent.upgrade().unwrap();
            }
        }

        // set children to be zombie and reparent them to init.
        debug_assert_ne!(self.tid(), INITPROC_PID);
        let children = self.children.lock();
        if !children.is_empty() {
            let init = TASK_MANAGER.get(INITPROC_PID).unwrap();
            children.values().for_each(|c| {
                c.set_zombie();
                *c.parent.lock() = Some(Arc::downgrade(&init));
            });
            init.children.lock().extend(children.clone());
        }
        drop(children);

        // release all fd

        // NOTE: leader will be removed by parent calling `sys_wait4`
        if !self.is_leader() {
            self.with_mut_thread_group(|tg| tg.remove(self));
            TASK_MANAGER.remove(self)
        }
    }

    with_!(children, BTreeMap<Tid, Arc<Task>>);
    with_!(memory_space, MemorySpace);
    with_!(thread_group, ThreadGroup);
}

/// Hold a group of threads which belongs to the same process.
pub struct ThreadGroup {
    members: BTreeMap<Tid, Weak<Task>>,
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
        self.members.insert(leader.tid(), Arc::downgrade(&leader));
    }

    pub fn push(&mut self, task: Arc<Task>) {
        debug_assert!(self.leader.is_some());
        self.members.insert(task.tid(), Arc::downgrade(&task));
    }

    pub fn remove(&mut self, thread: &Task) {
        debug_assert!(self.leader.is_some());
        self.members.remove(&thread.tid());
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn tgid(&self) -> Tid {
        self.leader.as_ref().unwrap().upgrade().unwrap().tid()
    }

    pub fn iter(&self) -> impl Iterator<Item = Arc<Task>> + '_ {
        self.members.values().map(|t| t.upgrade().unwrap())
    }
}
