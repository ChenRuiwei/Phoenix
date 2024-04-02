use config::process::INITPROC_PID;
use recycle_allocator::RecycleAllocator;
use sync::mutex::SpinNoIrqLock;

use crate::stack_trace;

static PID_ALLOCATOR: SpinNoIrqLock<RecycleAllocator> =
    SpinNoIrqLock::new(RecycleAllocator::new(INITPROC_PID));

pub type Pid = usize;

#[derive(Debug)]
pub struct PidHandle(pub Pid);

impl Drop for PidHandle {
    fn drop(&mut self) {
        stack_trace!();
        log::debug!("drop pid {}", self.0);
        PID_ALLOCATOR.lock().dealloc(self.0);
    }
}

pub fn alloc_pid() -> PidHandle {
    stack_trace!();
    PidHandle(PID_ALLOCATOR.lock().alloc())
}
