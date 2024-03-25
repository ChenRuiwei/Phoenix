//! Trap handling functionality
mod ctx;
/// Kernel trap handler
pub mod kernel_trap;
/// User trap handler
pub mod user_trap;

use core::arch::global_asm;

use arch::interrupts::set_trap_handler;

global_asm!(include_str!("trap.S"));

extern "C" {
    fn __trap_from_user();
    fn __trap_from_kernel();
}

pub fn init() {
    set_kernel_trap_entry();
}

///
pub fn set_kernel_trap_entry() {
    set_trap_handler(__trap_from_kernel as usize);
}

fn set_user_trap_entry() {
    set_trap_handler(__trap_from_user as usize);
}

pub use ctx::{TrapContext, UserContext};
