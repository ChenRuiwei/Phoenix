//! Implementation of [`TidAllocator`]
use config::process::INITPROC_PID;
use log::{debug, warn};
use recycle_allocator::RecycleAllocator;
use sync::mutex::SpinNoIrqLock;

use crate::{futex::futex_wake, mm::user_check::UserCheck, processor::SumGuard, stack_trace};

static TID_ALLOCATOR: SpinNoIrqLock<RecycleAllocator> =
    SpinNoIrqLock::new(RecycleAllocator::new(INITPROC_PID));
/// Bind pid lifetime to `TidHandle`
pub struct TidHandle(pub usize);

impl Drop for TidHandle {
    fn drop(&mut self) {
        stack_trace!();
        debug!("drop pid {}", self.0);
        // println!("\u{1B}[33m drop pid {} \u{1B}[0m", self.0);
        TID_ALLOCATOR.lock().dealloc(self.0);
    }
}
/// Allocate a pid from PID_ALLOCATOR
pub fn tid_alloc() -> TidHandle {
    stack_trace!();
    TidHandle(TID_ALLOCATOR.lock().alloc())
}

/// Tid address which may be set by `set_tid_address` syscall
pub struct TidAddress {
    /// Set tid address
    pub set_tid_address: Option<usize>,
    /// Clear tid address
    pub clear_tid_address: Option<usize>,
}

impl TidAddress {
    ///
    pub fn new() -> Self {
        stack_trace!();
        Self {
            set_tid_address: None,
            clear_tid_address: None,
        }
    }

    ///
    pub fn thread_died(&self) {
        stack_trace!();
        if let Some(clear_tid_address) = self.clear_tid_address {
            log::info!("Drop tid address {:#x}", clear_tid_address);
            if UserCheck::new()
                .check_writable_slice(clear_tid_address as *mut u8, core::mem::size_of::<usize>())
                .is_ok()
            {
                let _sum_guard = SumGuard::new();
                unsafe {
                    *(clear_tid_address as *mut usize) = 0;
                }
            }
            if futex_wake(clear_tid_address, 1).is_err() {
                warn!("futex wake failed when thread died");
            }
        }
    }
}
