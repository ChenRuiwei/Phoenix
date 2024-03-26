use alloc::{boxed::Box, sync::Arc};
use core::arch::asm;

use arch::interrupts::{disable_interrupt, enable_interrupt};
use config::processor::HART_NUM;
use riscv::register::sstatus::{self, FS};
use spin::Once;
use sync::cell::SyncUnsafeCell;

use super::{
    ctx::{EnvContext, LocalContext},
    current_trap_cx,
};
use crate::{
    mm::{self, PageTable},
    process::thread::Thread,
    stack_trace,
};

const HART_EACH: Hart = Hart::new();
pub static mut HARTS: [Hart; HART_NUM] = [HART_EACH; HART_NUM];

/// The processor has several `Hart`s
pub struct Hart {
    hart_id: usize,
    local_ctx: Once<Box<LocalContext>>,
}

impl Hart {
    pub fn env(&self) -> &EnvContext {
        self.local_ctx().env()
    }
    pub fn env_mut(&mut self) -> &mut EnvContext {
        self.local_ctx_mut().env_mut()
    }
    pub fn local_ctx(&self) -> &LocalContext {
        debug_assert!(self.local_ctx.is_completed());
        self.local_ctx.get().unwrap()
    }
    pub fn local_ctx_mut(&mut self) -> &mut LocalContext {
        debug_assert!(self.local_ctx.is_completed());
        self.local_ctx.get_mut().unwrap()
    }
    pub fn current_task(&self) -> &Arc<Thread> {
        // TODO: add debug assert to ensure now the hart must have a task
        // assert_ne!(self.local_ctx.task_ctx())
        stack_trace!();
        &self.local_ctx().task_ctx().thread
    }
    pub fn is_idle(&self) -> bool {
        self.local_ctx().is_idle()
    }
    pub fn change_page_table(&mut self, page_table: Arc<SyncUnsafeCell<PageTable>>) {
        stack_trace!();
        let task_ctx = self.local_ctx_mut().task_ctx_mut();
        task_ctx.page_table = page_table;
    }
}

impl Hart {
    pub const fn new() -> Self {
        Hart {
            hart_id: 0,
            local_ctx: Once::new(),
        }
    }
    pub fn init_local_ctx(&self) {
        self.local_ctx
            .call_once(|| Box::new(LocalContext::new(None)));
    }
    pub fn set_hart_id(&mut self, hart_id: usize) {
        self.hart_id = hart_id;
    }
    pub fn hart_id(&self) -> usize {
        self.hart_id
    }
    /// Change thread(task) context,
    /// Now only change page table temporarily
    pub fn enter_user_task_switch(&mut self, task: &mut Box<LocalContext>) {
        // self can only be an executor running
        assert!(self.is_idle());
        assert!(!task.is_idle());

        disable_interrupt();
        let new_env = task.env();
        let old_env = self.env();
        let sie = EnvContext::env_change(new_env, old_env);

        unsafe {
            (*task.task_ctx().page_table.get()).activate();
            (*task.task_ctx().thread.inner.get())
                .time_info
                .when_entering()
        }
        core::mem::swap(self.local_ctx_mut(), task);
        if sie {
            enable_interrupt();
        }
    }
    pub fn leave_user_task_switch(&mut self, task: &mut Box<LocalContext>) {
        disable_interrupt();

        let new_env = task.env();
        let old_env = self.env();
        let sie = EnvContext::env_change(new_env, old_env);

        // Save float regs
        current_trap_cx().user_fx.yield_task();

        mm::activate_kernel_space();
        core::mem::swap(self.local_ctx_mut(), task);

        unsafe {
            (*task.task_ctx().thread.inner.get())
                .time_info
                .when_leaving()
        }
        if sie {
            enable_interrupt();
        }
    }
    pub fn kernel_task_switch(&mut self, task: &mut Box<LocalContext>) {
        disable_interrupt();

        let new_env = task.env();
        let old_env = self.env();
        let sie = EnvContext::env_change(new_env, old_env);
        core::mem::swap(self.local_ctx_mut(), task);

        if sie {
            enable_interrupt();
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

pub fn set_hart_stack() {
    let h = local_hart();
    let sp: usize;
    unsafe {
        asm!("mv {}, sp", out(reg) sp);
    }
    println!("[kernel][hart{}] set_hart_stack: sp {:#x}", h.hart_id, sp);
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
    set_hart_stack();
    unsafe {
        sstatus::set_fs(FS::Initial);
    }
    println!("init hart {} finished", hart_id);
}
