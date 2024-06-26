use alloc::sync::Arc;
use core::arch::asm;

use arch::interrupts::{disable_interrupt, enable_interrupt};
use config::processor::HART_NUM;
use riscv::register::sstatus::{self, FS};

use super::env::EnvContext;
use crate::{
    mm,
    task::{Pid, Task},
};

const HART_EACH: Hart = Hart::new();
pub static mut HARTS: [Hart; HART_NUM] = [HART_EACH; HART_NUM];

/// Each cpu owns one `Hart`.
pub struct Hart {
    hart_id: usize,
    task: Option<Arc<Task>>,
    env: EnvContext,
    pub last_task_pid: Pid,
}

impl Hart {
    pub const fn new() -> Self {
        Hart {
            hart_id: 0,
            task: None,
            env: EnvContext::new(),
            last_task_pid: 0,
        }
    }

    pub fn hart_id(&self) -> usize {
        self.hart_id
    }

    pub fn task(&self) -> &Arc<Task> {
        self.task.as_ref().unwrap()
    }

    fn set_task(&mut self, task: Arc<Task>) {
        self.task = Some(task);
    }

    fn clear_task(&mut self) {
        self.task = None;
    }

    pub fn has_task(&self) -> bool {
        self.task.is_some()
    }

    pub fn env(&self) -> &EnvContext {
        &self.env
    }

    pub fn env_mut(&mut self) -> &mut EnvContext {
        &mut self.env
    }

    fn change_env(&self, env: &EnvContext) {
        self.env().change_env(env);
    }

    pub fn set_hart_id(&mut self, hart_id: usize) {
        self.hart_id = hart_id;
    }

    /// Change thread context.
    ///
    /// Now only change page table temporarily
    pub fn enter_user_task_switch(&mut self, task: &mut Arc<Task>, env: &mut EnvContext) {
        // self can only be an executor running
        debug_assert!(self.task.is_none());
        unsafe { disable_interrupt() };
        unsafe { env.auto_sum() };
        self.set_task(Arc::clone(task));
        task.time_stat().record_switch_in();
        core::mem::swap(self.env_mut(), env);
        // PERF: do not switch page table if it belongs to the same user
        // PERF: support ASID for page table
        if task.pid() != self.last_task_pid {
            unsafe { task.switch_page_table() };
        } else {
            // log::warn!("优化了，此时不需要切换页表");
        }
        unsafe { task.switch_page_table() };
        unsafe { enable_interrupt() };
        log::trace!("[enter_user_task_switch] enter user task");
    }

    pub fn leave_user_task_switch(&mut self, env: &mut EnvContext) {
        log::trace!("[leave_user_task_switch] leave user task");
        unsafe { disable_interrupt() };
        unsafe { env.auto_sum() };
        // PERF: no need to switch to kernel page table
        // unsafe { mm::switch_kernel_page_table() };
        core::mem::swap(self.env_mut(), env);
        self.task().time_stat().record_switch_out();
        self.last_task_pid = self.task().pid();
        self.clear_task();
        unsafe { enable_interrupt() };
    }

    pub fn kernel_task_switch(&mut self, env: &mut EnvContext) {
        unsafe { disable_interrupt() };
        self.change_env(env);
        core::mem::swap(self.env_mut(), env);
        unsafe { enable_interrupt() };
    }

    pub fn enter_preempt_switch(&mut self) -> Self {
        self.env.preempt_record();
        let mut new = Self::new();
        new.hart_id = self.hart_id;
        new.task = None;
        new.env = EnvContext::new();
        unsafe { new.env.auto_sum() };
        core::mem::swap(&mut new, self);
        new
    }

    pub fn leave_preempt_switch(&mut self, old_hart: &mut Hart) {
        core::mem::swap(self, old_hart);
        unsafe { self.env.preempt_resume() };
        unsafe { self.env.auto_sum() }
    }
}

unsafe fn get_hart(hart_id: usize) -> &'static mut Hart {
    &mut HARTS[hart_id]
}

/// Set hart control block according to `hard_id` and set register tp points to
/// the hart control block.
pub unsafe fn set_local_hart(hart_id: usize) {
    let hart = get_hart(hart_id);
    hart.set_hart_id(hart_id);
    let hart_addr = hart as *const _ as usize;
    asm!("mv tp, {}", in(reg) hart_addr);
}

/// Get the current `Hart` by `tp` register.
pub fn local_hart() -> &'static mut Hart {
    unsafe {
        let tp: usize;
        asm!("mv {}, tp", out(reg) tp);
        &mut *(tp as *mut Hart)
    }
}

pub fn init(hart_id: usize) {
    unsafe {
        set_local_hart(hart_id);
        sstatus::set_fs(FS::Initial);
    }
}

pub fn current_task() -> Arc<Task> {
    local_hart().task().clone()
}

/// WARN: never hold a local task ref when it may get scheduled, will cause bug
/// on smp situations.
///
/// ```rust
/// let task = current_task_ref();
/// task.do_something(); // the task ref is hart0's task
/// yield_now().await();
/// task.do_something(); // the task is still hart0's task, the two tasks may be different!
/// ```
pub fn current_task_ref() -> &'static Arc<Task> {
    local_hart().task()
}
