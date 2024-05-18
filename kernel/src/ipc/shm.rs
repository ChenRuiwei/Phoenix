use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};

use arch::time::get_time_sec;
use config::mm::PAGE_SIZE;
use hashbrown::HashMap;
use memory::{page::Page, VirtAddr};
use recycle_allocator::RecycleAllocator;
use spin::Lazy;
use sync::mutex::SpinNoIrqLock;

use super::IpcPerm;
use crate::{mm::memory_space::vm_area::MapPerm, task::Task};

pub struct SharedMemory {
    pub shmid_ds: ShmIdDs,
    pub pages: Vec<Weak<Page>>,
}

#[repr(C)]
pub struct ShmIdDs {
    // Ownership and permissions
    pub shm_perm: IpcPerm,
    // Size of segment (bytes)
    pub shm_segsz: usize,
    // Last attach time
    pub shm_atime: usize,
    // Last detach time
    pub shm_dtime: usize,
    // Creation time/time of last modification via shmctl()
    pub shm_ctime: usize,
    // PID of creator
    pub shm_cpid: usize,
    // PID of last shmat(2)/shmdt(2)
    pub shm_lpid: usize,
    // No. of current attaches
    pub shm_nattch: usize,
}

impl ShmIdDs {
    pub fn new(sz: usize, cpid: usize) -> Self {
        Self {
            shm_perm: IpcPerm::default(),
            shm_segsz: sz,
            shm_atime: 0,
            shm_dtime: 0,
            shm_ctime: get_time_sec(),
            shm_cpid: cpid,
            shm_lpid: 0,
            shm_nattch: 0,
        }
    }
    pub fn attach(&mut self, lpid: usize) {
        // shm_atime is set to the current time.
        self.shm_atime = get_time_sec();
        // shm_lpid is set to the process-ID of the calling process.
        self.shm_lpid = lpid;
        // shm_nattch is incremented by one.
        self.shm_nattch += 1;
    }

    /// return whether the SHARED_MEMORY_MANAGER should remove the SharedMemory
    /// which self ShmIdDs belongs to;
    pub fn detach(&mut self, lpid: usize) -> bool {
        // shm_dtime is set to the current time.
        self.shm_dtime = get_time_sec();
        // shm_lpid is set to the process-ID of the calling process.
        self.shm_lpid = lpid;
        // shm_nattch is decremented by one.
        self.shm_nattch -= 1;
        debug_assert!(self.shm_nattch >= 0);
        if self.shm_nattch == 0 {
            return true;
        }
        false
    }
}

impl SharedMemory {
    pub fn new(sz: usize, pid: usize) -> Self {
        Self {
            shmid_ds: ShmIdDs::new(sz, pid),
            pages: Vec::with_capacity(sz / PAGE_SIZE + 1),
        }
    }
    /// attach aligned `shm_va` to `task` with permission `map_perm`.
    pub fn attach(
        &mut self,
        task: &Arc<Task>,
        shm_va: VirtAddr,
        shm_id: usize,
        map_perm: MapPerm,
    ) -> VirtAddr {
        let ret_addr = task.with_mut_memory_space(|m| {
            m.attach_shm(self.size(), shm_va, map_perm, &mut self.pages)
        });
        task.with_mut_shm_ids(|ids| {
            ids.insert(ret_addr.clone(), shm_id);
        });
        self.shmid_ds.attach(task.pid());
        ret_addr
    }
    pub fn size(&self) -> usize {
        self.shmid_ds.shm_segsz
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
