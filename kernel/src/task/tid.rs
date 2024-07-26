use config::process::INIT_PROC_PID;
use recycle_allocator::RecycleAllocator;
use sync::mutex::SpinNoIrqLock;

pub static TID_ALLOCATOR: SpinNoIrqLock<RecycleAllocator> =
    SpinNoIrqLock::new(RecycleAllocator::new(INIT_PROC_PID));

pub type Tid = usize;
pub type Pid = Tid;
pub type PGid = Tid;

#[derive(Debug)]
pub struct TidHandle(pub Tid);

impl Drop for TidHandle {
    fn drop(&mut self) {
        TID_ALLOCATOR.lock().dealloc(self.0);
    }
}

pub fn alloc_tid() -> TidHandle {
    TidHandle(TID_ALLOCATOR.lock().alloc())
}

/// Tid address which may be set by `set_tid_address` syscall.
pub struct TidAddress {
    /// When set, when spawning a new thread, the kernel sets the thread's tid
    /// to this address.
    pub set_child_tid: Option<usize>,
    /// When set, when the thread exits, the kernel sets the thread's tid to
    /// this address, and wake up a futex waiting on this address.
    pub clear_child_tid: Option<usize>,
}

impl TidAddress {
    pub const fn new() -> Self {
        Self {
            set_child_tid: None,
            clear_child_tid: None,
        }
    }
}
