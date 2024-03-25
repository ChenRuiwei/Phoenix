use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
    vec::Vec,
};

use config::mm::{PAGE_SIZE, PAGE_SIZE_BITS};
use memory::{page_table::PTEFlags, MapPermission, VirtAddr};
use recycle_allocator::RecycleAllocator;
use sync::mutex::SpinNoIrqLock;
use systype::{GeneralRet, SyscallErr};

use super::Page;
use crate::{
    mm::{memory_space::vm_area::VmAreaType, PageBuilder},
    processor::current_process,
    stack_trace,
};

pub struct SharedMemoryManager {
    id_allocator: RecycleAllocator,
    // key -> id
    key_map: BTreeMap<usize, usize>,
    // id -> shm
    shm_map: BTreeMap<usize, SharedMemory>,
}

impl SharedMemoryManager {
    pub const fn new() -> Self {
        Self {
            id_allocator: RecycleAllocator::new(0),
            key_map: BTreeMap::new(),
            shm_map: BTreeMap::new(),
        }
    }

    pub fn alloc(&mut self, key: usize, len: usize) -> usize {
        let id = self.id_allocator.alloc();
        self.key_map.insert(key, id);
        let page_cnt = len / PAGE_SIZE;
        let shm = SharedMemory::new(page_cnt);
        self.shm_map.insert(id, shm);
        id
    }

    /// Attach a vm area to the shm.
    /// Return start addr of that vm area.
    pub fn attach(&mut self, shm_id: usize, addr: Option<VirtAddr>) -> GeneralRet<usize> {
        let shm = self.shm_map.get_mut(&shm_id).ok_or(SyscallErr::EINVAL)?;
        shm.attach(addr)
    }

    #[allow(unused)]
    pub fn detach() {
        todo!()
    }
}

// bitflags! {
//     pub struct ShmFlag
// }

pub struct SharedMemory {
    pages: Vec<Weak<Page>>,
    page_cnt: usize,
}

impl SharedMemory {
    pub fn new(page_cnt: usize) -> Self {
        Self {
            page_cnt,
            pages: Vec::new(),
        }
    }

    /// Note that this method must be called by the current thread
    pub fn attach(&mut self, addr: Option<VirtAddr>) -> GeneralRet<usize> {
        stack_trace!();

        current_process().inner_handler(|proc| {
            // TODO: give user all permissions temporarily
            let permission =
                MapPermission::R | MapPermission::X | MapPermission::W | MapPermission::U;
            let mut vma = match addr {
                Some(addr) => proc
                    .memory_space
                    .allocate_spec_area(
                        self.page_cnt << PAGE_SIZE_BITS,
                        permission,
                        addr,
                        VmAreaType::Shm,
                    )?
                    .ok_or(SyscallErr::ENOMEM)?,
                None => proc
                    .memory_space
                    .allocate_area(self.page_cnt << PAGE_SIZE_BITS, permission, VmAreaType::Shm)
                    .ok_or(SyscallErr::ENOMEM)?,
            };
            debug_assert!(vma.end_vpn().0 - vma.start_vpn().0 == self.page_cnt);
            for (idx, vpn) in vma.vpn_range.into_iter().enumerate() {
                log::debug!(
                    "[SharedMemory::attach] attach vma, vpn {:#x}, pte flags {:?}",
                    vpn.0,
                    PTEFlags::from(permission)
                );
                let page = match self.pages.len() <= idx {
                    true => {
                        let page = Arc::new(PageBuilder::new().permission(permission).build());
                        self.pages.push(Arc::downgrade(&page));
                        page
                    }
                    false => match self.pages[idx].upgrade().as_ref() {
                        Some(page) => page.clone(),
                        None => {
                            let page = Arc::new(PageBuilder::new().permission(permission).build());
                            self.pages[idx] = Arc::downgrade(&page);
                            page
                        }
                    },
                };
                vma.map_one(vpn, Some(page));
            }
            log::info!(
                "[SharedMemory::attach] attach vma, start vpn {:#x}, pte flags {:?}",
                vma.start_vpn().0,
                PTEFlags::from(permission)
            );
            let start_vpn = vma.start_vpn();
            proc.memory_space.insert_area(vma);
            proc.memory_space.activate();

            Ok(VirtAddr::from(start_vpn).0)
        })
    }
}

type Mutex<T> = SpinNoIrqLock<T>;

/// Global shared memory manager
pub static SHARED_MEMORY_MANAGER: Mutex<SharedMemoryManager> =
    Mutex::new(SharedMemoryManager::new());