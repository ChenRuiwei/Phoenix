use riscv::register::{
    sie, sstatus,
    stvec::{self, TrapMode},
};

#[inline]
pub fn is_interrupt_enabled() -> bool {
    sstatus::read().sie()
}

#[inline]
pub fn enable_interrupt() {
    unsafe {
        sstatus::set_sie();
    }
}

#[inline]
pub fn disable_interrupt() {
    unsafe { sstatus::clear_sie() }
}

#[inline]
pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

#[inline]
pub fn enable_external_interrupt() {
    unsafe {
        sie::set_sext();
    }
}

#[inline]
pub fn set_trap_handler(handler_addr: usize) {
    unsafe {
        stvec::write(handler_addr, TrapMode::Direct);
    }
}

/// Disable interrupt and resume to the intertupt state before when it gets
/// dropped
pub struct InterruptGuard {
    interrupt_before: bool,
}

impl InterruptGuard {
    pub fn new() -> Self {
        let interrupt_before = is_interrupt_enabled();
        disable_interrupt();
        Self { interrupt_before }
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        if self.interrupt_before {
            enable_interrupt();
        }
    }
}
