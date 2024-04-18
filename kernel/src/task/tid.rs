use config::process::INIT_PROC_PID;
use recycle_allocator::RecycleAllocator;
use sync::mutex::SpinNoIrqLock;

static TID_ALLOCATOR: SpinNoIrqLock<RecycleAllocator> =
    SpinNoIrqLock::new(RecycleAllocator::new(INIT_PROC_PID));

pub type Tid = usize;
pub type Pid = Tid;
pub type PGid = Tid;

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
