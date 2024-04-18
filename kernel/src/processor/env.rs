use riscv::register::sstatus;

use super::hart::local_hart;

/// use RAII to guard `sum` flag.
pub struct SumGuard;

impl SumGuard {
    pub fn new() -> Self {
        local_hart().env_mut().inc_sum();
        Self {}
    }
}

impl Drop for SumGuard {
    fn drop(&mut self) {
        local_hart().env_mut().dec_sum();
    }
}

/// Store some permission flags
pub struct EnvContext {
    /// Permit supervisor user memory access
    sum_cnt: usize,
}

impl EnvContext {
    pub const fn new() -> Self {
        Self { sum_cnt: 0 }
    }

    unsafe fn auto_sum(&self) {
        if self.sum_cnt == 0 {
            sstatus::clear_sum();
        } else {
            sstatus::set_sum();
        }
    }

    pub fn inc_sum(&mut self) {
        self.sum_cnt += 1;
        unsafe { self.auto_sum() };
    }

    pub fn dec_sum(&mut self) {
        debug_assert!(self.sum_cnt > 0);
        self.sum_cnt -= 1;
        unsafe { self.auto_sum() };
    }

    pub fn change_env(&self, new: &Self) {
        unsafe { new.auto_sum() };
    }
}
