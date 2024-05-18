use alloc::{sync::Weak, vec::Vec};
use core::time::Duration;

use arch::time::get_time_duration;
use config::mm::PAGE_SIZE;
use hashbrown::HashMap;
use memory::page::Page;
use recycle_allocator::RecycleAllocator;
use spin::Lazy;
use sync::mutex::SpinNoIrqLock;

use super::IpcPerm;

pub struct SharedMemory {
    pub shmid_ds: ShmIdDs,
    pub pages: Vec<Weak<Page>>,
}

pub struct ShmIdDs {
    // Ownership and permissions
    pub shm_perm: IpcPerm,
    // Size of segment (bytes)
    pub shm_segsz: usize,
    // Last attach time
    pub shm_atime: Duration,
    // Last detach time
    pub shm_dtime: Duration,
    // Creation time/time of last modification via shmctl()
    pub shm_ctime: Duration,
    // PID of creator
    pub shm_cpid: usize,
    // PID of last shmat(2)/shmdt(2)
    pub shm_lpid: usize,
    // No. of current attaches
    pub shm_nattch: usize,
}

impl ShmIdDs {
    pub fn new(sz: usize, pid: usize) -> Self {
        Self {
            shm_perm: IpcPerm::default(),
            shm_segsz: sz,
            shm_atime: Duration::ZERO,
            shm_dtime: Duration::ZERO,
            shm_ctime: get_time_duration(),
            shm_cpid: pid,
            shm_lpid: 0,
            shm_nattch: 0,
        }
    }
}

impl SharedMemory {
    pub fn new(sz: usize, pid: usize) -> Self {
        Self {
            shmid_ds: ShmIdDs::new(sz, pid),
            pages: Vec::with_capacity(sz / PAGE_SIZE + 1),
        }
    }
}

pub struct SharedMemoryManager(pub SpinNoIrqLock<HashMap<usize, SharedMemory>>);
impl SharedMemoryManager {
    pub fn init() -> Self {
        Self(SpinNoIrqLock::new(HashMap::new()))
    }
}

pub static SHARED_MEMORY_MANAGER: Lazy<SharedMemoryManager> = Lazy::new(SharedMemoryManager::init);
pub static SHARED_MEMORY_KEY_ALLOCATOR: SpinNoIrqLock<RecycleAllocator> =
    SpinNoIrqLock::new(RecycleAllocator::new(2));
bitflags! {
    #[derive(Debug)]
    pub struct ShmGetFlags: i32 {
        /// Create a new segment. If this flag is not used, then shmget() will find the segment associated with key and check to see if the user has permission to access the segment.
        const IPC_CREAT = 0o1000;
        /// This flag is used with IPC_CREAT to ensure that this call creates the segment.  If the segment already exists, the call fails.
        const IPC_EXCL = 0o2000;
    }
}

impl From<usize> for ShmGetFlags {
    fn from(value: usize) -> Self {
        ShmGetFlags::from_bits_truncate(value as i32)
    }
}

bitflags! {
    #[derive(Debug)]
    pub struct ShmAtFlags: i32 {
        /// Attach the segment for read-only access.If this flag is not specified, the segment is attached for read and write access, and the process must have read and write permission for  the  segment.
        const SHM_RDONLY = 0o10000;
        /// round attach address to SHMLBA boundary
        const SHM_RND = 0o20000;
        /// take-over region on attach
        const SHM_REMAP = 0o40000;
        /// Allow the contents of the segment to be executed.  The caller must have execute permission on the segment.
        const SHM_EXEC = 0o100000;
    }
}

impl From<usize> for ShmAtFlags {
    fn from(value: usize) -> Self {
        ShmAtFlags::from_bits_truncate(value as i32)
    }
}
