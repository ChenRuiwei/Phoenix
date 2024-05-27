use arch::memory::sfence_vma_all;
use riscv::register::{satp, sepc};

use super::hart::local_hart;

/// use RAII to guard `sum` flag.
pub struct SumGuard;

impl SumGuard {
    pub fn new() -> Self {
        local_hart().env_mut().inc_sum();
        Self
    }
}

impl Drop for SumGuard {
    fn drop(&mut self) {
        local_hart().env_mut().dec_sum();
    }
}

pub fn within_sum<T>(f: impl FnOnce() -> T) -> T {
    let _guard = SumGuard::new();
    let ret = f();
    ret
}

/// Store some permission flags
pub struct EnvContext {
    // For preempt and non preempt
    /// Permit supervisor user memory access
    sum_cnt: usize,

    // For preempt only
    sstatus: usize,
    sepc: usize,
    satp: usize,
}

impl EnvContext {
    pub const fn new() -> Self {
        Self {
            sum_cnt: 0,
            sstatus: 0,
            sepc: 0,
            satp: 0,
        }
    }

    pub unsafe fn auto_sum(&self) {
        if self.sum_cnt == 0 {
            riscv::register::sstatus::clear_sum();
        } else {
            riscv::register::sstatus::set_sum();
        }
    }

    pub fn inc_sum(&mut self) {
        if self.sum_cnt == 0 {
            unsafe { riscv::register::sstatus::set_sum() };
        }
        self.sum_cnt += 1;
    }

    pub fn dec_sum(&mut self) {
        debug_assert!(self.sum_cnt > 0);
        self.sum_cnt -= 1;
        if self.sum_cnt == 0 {
            unsafe { riscv::register::sstatus::clear_sum() };
        }
    }

    pub fn change_env(&self, new: &Self) {
        unsafe { new.auto_sum() };
    }

    pub fn preempt_record(&mut self) {
        self.sstatus = arch::sstatus::read().bits();
        self.sepc = sepc::read();
        self.satp = satp::read().bits();
    }

    pub unsafe fn preempt_resume(&self) {
        arch::sstatus::write(self.sstatus);
        sepc::write(self.sepc);
        satp::write(self.satp);
        sfence_vma_all();
    }
}
