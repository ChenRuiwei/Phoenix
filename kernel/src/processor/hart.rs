use alloc::sync::Arc;
use core::arch::asm;

use arch::{
    interrupts::{disable_interrupt, enable_interrupt},
    time::get_time_duration,
};
use config::processor::HART_NUM;
use riscv::register::sstatus::{self, FS};

use super::ctx::EnvContext;
use crate::{
    mm::{self},
    task::Task,
    trap::TrapContext,
};

const HART_EACH: Hart = Hart::new();
pub static mut HARTS: [Hart; HART_NUM] = [HART_EACH; HART_NUM];

/// The processor has several `Hart`s
pub struct Hart {
    hart_id: usize,
    task: Option<Arc<Task>>,
    env: EnvContext,
}

impl Hart {
    pub fn env(&self) -> &EnvContext {
        &self.env
    }
    pub fn env_mut(&mut self) -> &mut EnvContext {
        &mut self.env
    }
    pub fn current_task(&self) -> &Arc<Task> {
        self.task.as_ref().unwrap()
    }
}

impl Hart {
    pub const fn new() -> Self {
        Hart {
            hart_id: 0,
            task: None,
            env: EnvContext::new(),
        }
    }
    pub fn set_hart_id(&mut self, hart_id: usize) {
        self.hart_id = hart_id;
    }
    pub fn hart_id(&self) -> usize {
        self.hart_id
    }

    pub fn has_task(&self) -> bool {
        self.task.is_some()
    }

    /// Change thread context,
    ///
    /// Now only change page table temporarily
    pub fn enter_user_task_switch(&mut self, task: &mut Arc<Task>, env: &mut EnvContext) {
        // self can only be an executor running
        debug_assert!(self.task.is_none());
        unsafe { disable_interrupt() };
        let old_env = self.env();
        let sie = EnvContext::env_change(env, old_env);
        set_current_task(Arc::clone(task));
        task.get_time_stat()
            .record_switch_in_time(get_time_duration());
        // task.time_stat.record_switch_in_time(get_time_duration());
        core::mem::swap(self.env_mut(), env);
        unsafe { task.switch_page_table() };
        if sie {
            unsafe { enable_interrupt() };
        }
    }

    pub fn leave_user_task_switch(&mut self, env: &mut EnvContext) {
        unsafe { disable_interrupt() };
        let old_env = self.env();
        let sie = EnvContext::env_change(env, old_env);
        unsafe { mm::switch_kernel_page_table() };
        core::mem::swap(self.env_mut(), env);
        self.task
            .as_ref()
            .unwrap()
            .get_time_stat()
            .record_switch_out_time(get_time_duration());
        self.task = None;
        if sie {
            unsafe { enable_interrupt() };
        }
    }

    pub fn kernel_task_switch(&mut self, env: &mut EnvContext) {
        unsafe { disable_interrupt() };
        let old_env = self.env();
        let sie = EnvContext::env_change(env, old_env);
        core::mem::swap(self.env_mut(), env);
        if sie {
            unsafe { enable_interrupt() };
        }
    }
}

unsafe fn get_hart_by_id(hart_id: usize) -> &'static mut Hart {
    &mut HARTS[hart_id]
}

/// Set the cpu hart control block according to `hard_id`
/// set register tp points to hart control block
pub fn set_local_hart(hart_id: usize) {
    unsafe {
        let hart = get_hart_by_id(hart_id);
        hart.set_hart_id(hart_id);
        let hart_addr = hart as *const _ as usize;
        asm!("mv tp, {}", in(reg) hart_addr);
    }
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
    println!("start to init hart {}...", hart_id);
    set_local_hart(hart_id);
    unsafe {
        sstatus::set_fs(FS::Initial);
    }
    println!("init hart {} finished", hart_id);
}

pub fn local_env_mut() -> &'static mut EnvContext {
    local_hart().env_mut()
}

pub fn current_task() -> &'static Arc<Task> {
    local_hart().current_task()
}

pub fn set_current_task(task: Arc<Task>) {
    local_hart().task = Some(task);
}

pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task().trap_context_mut()
}
