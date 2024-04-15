use riscv::register::sstatus;

fn is_interrupt_enabled() -> bool {
    #[cfg(target_arch = "riscv64")]
    sstatus::read().sie()
}

unsafe fn enable_interrupt() {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        sstatus::set_sie();
    }
}

unsafe fn disable_interrupt() {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        sstatus::clear_sie()
    }
}

/// Disable interrupt and resume to the intertupt state before when it gets
/// dropped.
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
