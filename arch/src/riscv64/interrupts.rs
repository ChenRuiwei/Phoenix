use riscv::register::{
    sie, sstatus,
    stvec::{self, TrapMode},
};

#[inline]
pub fn is_interrupt_enabled() -> bool {
    sstatus::read().sie()
}

#[inline]
pub unsafe fn enable_interrupt() {
    #[cfg(feature = "irq")]
    sstatus::set_sie();
}

#[inline]
pub unsafe fn disable_interrupt() {
    #[cfg(feature = "irq")]
    sstatus::clear_sie();
}

#[inline]
pub unsafe fn enable_timer_interrupt() {
    sie::set_stimer();
}

#[inline]
pub unsafe fn enable_external_interrupt() {
    sie::set_sext();
}

#[inline]
pub unsafe fn set_trap_handler(handler_addr: usize) {
    stvec::write(handler_addr, TrapMode::Direct);
}

#[inline]
pub unsafe fn set_trap_handler_vector(handler_addr: usize) {
    stvec::write(handler_addr, TrapMode::Vectored);
}

/// Disable interrupt and resume to the interrupt state before when it gets
/// dropped
pub struct InterruptGuard {
    interrupt_before: bool,
}

impl InterruptGuard {
    pub fn new() -> Self {
        let interrupt_before = is_interrupt_enabled();
        unsafe { disable_interrupt() };
        Self { interrupt_before }
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        if self.interrupt_before {
            unsafe { enable_interrupt() };
        }
    }
}
