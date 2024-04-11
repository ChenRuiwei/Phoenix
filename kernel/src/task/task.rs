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

use config::mm::USER_STACK_SIZE;
use signal::Signal;
use sync::mutex::SpinNoIrqLock;

use super::tid::{Pid, Tid, TidHandle};
use crate::{
    mm::{
        memory_space::{
            self,
            vm_area::{MapPerm, VmArea},
        },
        MemorySpace,
    },
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
/// adopted by Linux.
pub struct Task {
    ///
    tid: TidHandle,
    /// Whether this task is a zombie. Locked because of other task may operate
    /// this state, e.g. execve will kill other tasks.
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
        });

        task.thread_group.lock().push_leader(task.clone());

        TASK_MANAGER.add(&task);
        log::debug!("create a new process, pid {}", task.tid());
        schedule::spawn_user_task(task);
    }

    fn parent(&self) -> Option<Weak<Self>> {
        self.parent.lock().clone()
    }

    pub fn is_leader(&self) -> bool {
        self.pid() == self.tid()
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
    pub fn do_clone(&self) {}

    // TODO:
    /// All threads other than the calling thread are destroyed during an
    /// execve().
    pub fn do_execve(&self, elf_data: &[u8], argv: Vec<String>, envp: Vec<String>) {
        // change memory space
        let mut memory_space = MemorySpace::new_user();
        let (entry, auxv) = memory_space.parse_and_map_elf(elf_data);
        self.with_mut_memory_space(|m| *m = memory_space);
        unsafe { self.switch_page_table() };

        // exit all threads except main
        self.with_thread_group(|tg| {
            for t in tg.iter() {
                if !tg.is_leader(&t) {
                    t.set_zombie();
                }
            }
        });

        // alloc stack, and push argv, envp and auxv
        let stack_begin = self.with_mut_memory_space(|m| m.alloc_stack(USER_STACK_SIZE));

        // alloc heap

        // init trap context
        self.trap_context_mut()
            .init_user(stack_begin.into(), entry, 0, 0, 0);
    }

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
    with_!(thread_group, ThreadGroup);
}

impl Drop for Task {
    fn drop(&mut self) {
        log::info!("task {} died!", self.tid());
    }
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

    pub fn is_leader(&self, task: &Task) -> bool {
        self.tgid() == task.tid()
    }

    pub fn tgid(&self) -> Tid {
        self.leader.as_ref().unwrap().upgrade().unwrap().tid()
    }

    pub fn iter(&self) -> impl Iterator<Item = Arc<Task>> + '_ {
        self.members.values().map(|t| t.upgrade().unwrap())
    }
}
