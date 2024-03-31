use alloc::{collections::BTreeMap, string::{String, ToString}, sync::{Arc, Weak}, vec::Vec};
use log::debug;
use sync::mutex::SpinNoIrqLock;
use core::{sync::atomic::AtomicUsize, task::Waker};
use crate::{fs::{FdTable, File}, futex::FutexQueue, mm::MemorySpace, net::SocketTable, process::resource::RLimit, stack_trace, task::{manager::TASK_MANAGER, pid::alloc_pid}, timer::ffi::ITimerVal, trap::TrapContext};

use super::pid::PidHandle;


pub struct Task {
    pub pid: PidHandle,
    pub inner: SpinNoIrqLock<TaskInner>,
}

/// Linux进程的状态https://zhuanlan.zhihu.com/p/343806496
pub enum TaskState{
    /// 就绪状态和运行状态
    TASK_RUNNING,
    TASK_ZOMBIE,
    // /// 可中断等待状态, 由于进程未获得它所申请的资源而处在等待状态。一旦资源有效或者有唤醒信号，进程会立即结束等待而进入就绪状态
    // TASK_INTERRUPTIBL,
    // /// 不可中断等待状态
    // TASK_UNINTERRUPTIBL,
}

pub struct TaskInner {
    /// "command"的缩写，即运行的程序或命令的名称。这个名称通常是启动进程的可执行文件的名字（模仿Linux）
    pub comm: String,
    /// Whether this process is a zombie process
    pub state: TaskState,
    /// The process's address space
    pub memory_space: MemorySpace,
    /// Parent process
    pub parent: Option<Weak<Task>>,
    /// Children processes
    pub children: Vec<Arc<Task>>,
    /// Exit code of the current process
    /// Note that we may need to put this member in every thread
    pub exit_code: i8,
    pub trap_context: TrapContext,
    pub waker: Option<Waker>,
    pub ustack_top: usize,
}

impl Task {
    pub fn pid(&self) -> usize {
        stack_trace!();
        self.pid.0
    }

    pub fn exit_code(&self) -> i8 {
        stack_trace!();
        self.inner.lock().exit_code
    }

    pub fn new_initproc(elf_data: &[u8], elf_file: Option<&Arc<dyn File>>) -> Arc<Self> {
        stack_trace!();
        let (memory_space, user_sp_top, entry_point, _auxv) =
            MemorySpace::from_elf(elf_data, elf_file);

        let task = Arc::new(Self {
            pid: alloc_pid(),
            inner: SpinNoIrqLock::new(TaskInner {
                comm: "Init".to_string(),
                state: TaskState::TASK_RUNNING,
                memory_space,
                parent: None,
                children: Vec::new(),
                exit_code: 0,
                trap_context: TrapContext::app_init_context(entry_point, user_sp_top),
                waker: None,
                ustack_top: user_sp_top,
            }),
        });

        TASK_MANAGER.add_task(task.pid(), &task);
        // PROCESS_GROUP_MANAGER.add_group(process.pgid());
        // Add the main thread into scheduler
        spawn_user_task(task);
        debug!("create a new task, pid {}", task.pid());
        task
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        stack_trace!();
        log::info!("task {} died!", self.pid());
    }
}

