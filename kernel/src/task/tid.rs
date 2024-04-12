use config::process::INITPROC_PID;
use recycle_allocator::RecycleAllocator;
use sync::mutex::SpinNoIrqLock;

static TID_ALLOCATOR: SpinNoIrqLock<RecycleAllocator> =
    SpinNoIrqLock::new(RecycleAllocator::new(INITPROC_PID));

pub type Tid = usize;
pub type Pid = usize;

#[derive(Debug)]
pub struct TidHandle(pub Tid);

impl Drop for TidHandle {
    fn drop(&mut self) {
        log::debug!("drop tid {}", self.0);
        TID_ALLOCATOR.lock().dealloc(self.0);
    }
}

pub fn alloc_tid() -> TidHandle {
    TidHandle(TID_ALLOCATOR.lock().alloc())
}
