//! Trap handling functionality

pub mod context;
pub mod kernel_trap;
pub mod user_trap;

use core::arch::global_asm;

use arch::interrupts::set_trap_handler;
pub use context::TrapContext;

global_asm!(include_str!("trap.asm"));

extern "C" {
    fn __trap_from_user();
    fn __trap_from_kernel();
}

pub fn init() {
    unsafe { set_kernel_trap() };
}

pub unsafe fn set_kernel_trap() {
    set_trap_handler(__trap_from_kernel as usize);
}

unsafe fn set_user_trap() {
    set_trap_handler(__trap_from_user as usize);
}
