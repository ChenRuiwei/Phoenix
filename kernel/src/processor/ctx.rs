use alloc::sync::Arc;

use riscv::register::sstatus;

use crate::utils::stack_trace::stack_tracker::StackTracker;

/// Store some permission flags
pub struct EnvContext {
    /// Supervisor interrupt disable
    sie_disabled: usize,
    /// Permit supervisor user memory access
    sum_enabled: usize,
    /// Stack tracker
    pub stack_tracker: StackTracker,
}

impl EnvContext {
    pub fn new() -> Self {
        Self {
            sie_disabled: 0,
            sum_enabled: 0,
            stack_tracker: StackTracker::new(),
        }
    }

    pub fn sum_inc(&mut self) {
        if self.sum_enabled == 0 {
            unsafe {
                sstatus::set_sum();
            }
        }
        self.sum_enabled += 1
    }

    pub fn sum_dec(&mut self) {
        if self.sum_enabled == 1 {
            unsafe {
                sstatus::clear_sum();
            }
        }
        self.sum_enabled -= 1
    }

    /// Return whether the new task should open kernel interrupt or not
    pub fn env_change(new: &Self, old: &Self) -> bool {
        unsafe {
            if (new.sum_enabled > 0) != (old.sum_enabled > 0) {
                if new.sum_enabled > 0 {
                    sstatus::set_sum();
                } else {
                    sstatus::clear_sum();
                }
            }
        }
        new.sie_disabled == 0
    }
}
