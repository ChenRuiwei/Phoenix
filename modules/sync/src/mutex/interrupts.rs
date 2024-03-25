use riscv::register::sstatus;

#[inline]
pub fn is_interrupt_enabled() -> bool {
    #[cfg(target_arch = "riscv64")]
    sstatus::read().sie()
}

#[inline]
pub fn enable_interrupt() {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        sstatus::set_sie();
    }
}

#[inline]
pub fn disable_interrupt() {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        sstatus::clear_sie()
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
