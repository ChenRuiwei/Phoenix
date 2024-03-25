use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec,
    vec::Vec,
};

use config::{
    board::MEMORY_END,
    mm::{
        DL_INTERP_OFFSET, KERNEL_DIRECT_OFFSET, MMAP_TOP, PAGE_SIZE, PAGE_SIZE_BITS,
        USER_STACK_SIZE,
    },
};
use log::{debug, error, info, trace, warn};
use memory::{
    address::SimpleRange, frame_alloc, page_table::PTEFlags, MapPermission, VPNRange, VirtAddr,
    VirtPageNum, MMIO,
};
use riscv::register::scause::Scause;
use sync::cell::SyncUnsafeCell;
use systype::{GeneralRet, SyscallErr};
use xmas_elf::ElfFile;

use self::{cow::CowPageManager, vm_area::VmAreaType};
pub use self::{
    page_fault_handler::{CowPageFaultHandler, PageFaultHandler, UStackPageFaultHandler},
    vm_area::VmArea,
};
use super::{Page, PageTable, PageTableEntry};
use crate::{
    fs::{resolve_path, File, OpenFlags, AT_FDCWD},
    mm::memory_space::{page_fault_handler::SBrkPageFaultHandler, vm_area::BackupFile},
    process::aux::*,
    processor::current_process,
    stack_trace,
};

///
pub mod page_fault_handler;
///
pub mod vm_area;

mod cow;

extern "C" {
    fn stext();
    fn strampoline();
    fn sigreturn_trampoline();
    fn etrampoline();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sstack();
    fn estack();
    fn sbss();
    fn ebss();
    fn ekernel();
}

/// Kernel Space for all processes
pub static mut KERNEL_SPACE: Option<MemorySpace> = None;

///
pub fn init_kernel_space() {
    stack_trace!();
    info!("start to init kernel space...");
    unsafe {
        KERNEL_SPACE = Some(MemorySpace::new_kernel());
    }
}

pub fn activate_kernel_space() {
    unsafe {
        KERNEL_SPACE
            .as_ref()
            .expect("KERNEL SPACE not init yet")
            .activate();
    }
}

/// Heap range
pub type HeapRange = SimpleRange<VirtAddr>;

///
#[derive(Clone)]
pub struct PageManager(pub BTreeMap<VirtPageNum, Arc<Page>>);

impl PageManager {
    ///
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
}

/// memory space structure, controls virtual-memory space
pub struct MemorySpace {
    /// we should ensure modifying page table exclusively(e.g. through
    /// process_inner's lock) TODO: optimization: decrease the lock
    /// granularity when handling page fault
    pub page_table: Arc<SyncUnsafeCell<PageTable>>,
    /// start vpn -> vm_area
    areas: SyncUnsafeCell<BTreeMap<VirtPageNum, VmArea>>,
    /// Cow page manager
    cow_pages: CowPageManager,
    /// heap range
    pub heap_range: Option<HeapRange>,
}

impl MemorySpace {
    /// Create an empty `MemorySpace`
    pub fn new_bare() -> Self {
        stack_trace!();
        let page_table = Arc::new(SyncUnsafeCell::new(PageTable::new()));
        Self {
            page_table,
            areas: SyncUnsafeCell::new(BTreeMap::new()),
            heap_range: None,
            cow_pages: CowPageManager::new(),
        }
    }

    /// Create an empty `MemorySpace` but owns the global kernel mapping
    pub fn new_from_global() -> Self {
        stack_trace!();
        let new_page_table = PageTable::from_global(unsafe {
            (*KERNEL_SPACE
                .as_ref()
                .expect("KERNEL SPACE not init yet")
                .page_table
                .get())
            .root_ppn
        });
        let page_table = Arc::new(SyncUnsafeCell::new(new_page_table));
        Self {
            page_table,
            areas: SyncUnsafeCell::new(BTreeMap::new()),
            heap_range: None,
            cow_pages: CowPageManager::new(),
        }
    }

    /// Get pagetable `root_ppn`
    #[allow(unused)]
    pub fn token(&self) -> usize {
        stack_trace!();
        self.page_table.get_unchecked_mut().token()
    }

    /// Clip the map areas overlapping with the given vpn range.
    /// Note that there may exist more than one area.
    /// Return the overlapping vma.
    pub fn clip_vm_areas_overlapping(
        &mut self,
        vpn_range: VPNRange,
    ) -> GeneralRet<Option<&VmArea>> {
        stack_trace!();
        let mut removed_areas: Vec<VirtPageNum> = Vec::new();
        let mut clipped_area: Option<VirtPageNum> = None;
        for (start_vpn, vma) in self
            .areas
            .get_mut()
            .range_mut(vpn_range.start()..vpn_range.end())
        {
            if vma.end_vpn() <= vpn_range.end() {
                // The vma is totally included by the given vpn range.
                // We should just remove it.
                removed_areas.push(*start_vpn);
                debug!("[clip_vm_areas_overlapping] remove vma {:?}", vma.vpn_range);
            } else {
                // Else, clip it.
                vma.clip(VPNRange::new(vpn_range.end(), vma.end_vpn()));
                debug!("[clip_vm_areas_overlapping] clip vma {:?}", vma.vpn_range);
                clipped_area = Some(*start_vpn);
            }
        }
        for start_vpn in removed_areas {
            self.areas.get_mut().remove(&start_vpn);
        }
        if let Some(clipped_area) = clipped_area {
            let vma = self.areas.get_mut().remove(&clipped_area).unwrap();
            let new_start_vpn = vma.start_vpn();
            self.areas.get_mut().insert(new_start_vpn, vma);
            return Ok(self.areas.get_mut().get(&new_start_vpn));
        }

        if let Some((_, vma)) = self.areas.get_mut().range_mut(..vpn_range.start()).last() {
            if vma.end_vpn() > vpn_range.start() {
                debug!("[clip_vm_areas_overlapping] clip vma {:?}", vma.vpn_range);
                vma.clip(VPNRange::new(vma.start_vpn(), vpn_range.start()));
                return Ok(Some(vma));
            }
        }

        Ok(None)
    }

    /// Remove vma by start vpn
    pub fn remove_vm_area(&mut self, start_vpn: VirtPageNum) -> Option<VmArea> {
        stack_trace!();
        self.areas.get_unchecked_mut().remove(&start_vpn)
    }

    /// Find the immutable ref of map area by the given vpn
    pub fn find_vm_area_by_vpn(&self, vpn: VirtPageNum) -> Option<&VmArea> {
        stack_trace!();
        // Range query to find the map area that this vpn belongs to
        // debug!("len before {}", self.areas.len());
        if let Some((_, vm_area)) = self.areas.get_unchecked_mut().range(..=vpn).next_back() {
            if vm_area.end_vpn() <= vpn {
                return None;
            }
            debug!(
                "[find_vm_area_by_vpn]: vpn {:#x} map area start {:#x} end {:#x}",
                vpn.0,
                vm_area.start_vpn().0,
                vm_area.end_vpn().0
            );
            // debug!("len after {}", self.areas.len());
            Some(vm_area)
        } else {
            None
        }
    }

    /// Find the mutable ref of map area by the given vpn
    pub fn find_vm_area_mut_by_vpn(&mut self, vpn: VirtPageNum) -> Option<&mut VmArea> {
        stack_trace!();
        if let Some(vma) = self.find_vm_area_mut_by_vpn_included(vpn) {
            if vma.end_vpn().0 == vpn.0 {
                None
            } else {
                Some(vma)
            }
        } else {
            None
        }
    }

    /// Find the mutable ref of map area by the given vpn(including end vpn)
    pub fn find_vm_area_mut_by_vpn_included(&mut self, vpn: VirtPageNum) -> Option<&mut VmArea> {
        stack_trace!();
        // Range query to find the map area that this vpn belongs to
        // debug!("len before {}", self.areas.len());
        if let Some((_, vm_area)) = self.areas.get_mut().range_mut(..=vpn).next_back() {
            if vm_area.end_vpn() < vpn {
                return None;
            }
            debug!(
                "vpn {:#x} map area start {:#x} end {:#x}",
                vpn.0,
                vm_area.start_vpn().0,
                vm_area.end_vpn().0
            );
            // debug!("len after {}", self.areas.len());
            Some(vm_area)
        } else {
            None
        }
    }

    /// Handle page fault synchronously.
    /// Return Some(handler) if async handle should be invoked.
    pub fn page_fault_handler(
        &self,
        va: VirtAddr,
        _scause: Scause,
    ) -> GeneralRet<(Arc<dyn PageFaultHandler>, Option<&VmArea>)> {
        stack_trace!();
        // There are serveral kinds of page faults:
        // 1. mmap area
        // 2. sbrk area
        // 3. user stack
        // 4. fork cow area
        // 5. execve elf file
        // 6. dynamic link
        // 7. illegal page fault
        // todo!()
        // find map area
        let vpn = va.floor();
        // First we should query from cow pages
        if self
            .cow_pages
            .page_mgr
            .get_unchecked_mut()
            .0
            .get(&va.floor())
            .is_some()
        {
            return self.cow_pages.page_fault_handler(va);
        }
        // Range query to find the map area that this vpn belongs to
        if let Some(vm_area) = self.find_vm_area_by_vpn(vpn) {
            // vm_area.handle_page_fault(va, page_table)
            // let page_table = unsafe { &mut (*self.page_table.get()) };
            vm_area.page_fault_handler(va)
        } else {
            warn!("memory set len {}", self.areas.get_unchecked_mut().len());
            for area in self.areas.get_unchecked_mut().iter() {
                log::warn!(
                    "area start vpn {:#x}, end vpn {:#x}",
                    area.0 .0,
                    area.1.end_vpn().0
                );
            }
            warn!("no such vma for va {:#x}, vpn {:#x}", va.0, vpn.0);
            Err(SyscallErr::EFAULT)
        }
    }

    /// Insert vm area lazily
    pub fn insert_area(&mut self, vma: VmArea) {
        stack_trace!();
        log::debug!("[insert_area] vpn range {:?}", vma.vpn_range);
        self.push_lazily(vma, None);
    }

    /// Assume that no conflicts.
    #[allow(unused)]
    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
        vma_type: VmAreaType,
    ) {
        stack_trace!();
        self.push(
            VmArea::new(
                start_va,
                end_va,
                MapType::Framed,
                permission,
                None,
                None,
                self.page_table.clone(),
                vma_type,
            ),
            0,
            None,
        );
    }

    /// Insert framed area without allocating physical memory
    #[allow(unused)]
    pub fn insert_framed_area_lazily(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
        handler: Option<Arc<dyn PageFaultHandler>>,
        vma_type: VmAreaType,
    ) {
        stack_trace!();
        self.push_lazily(
            VmArea::new(
                start_va,
                end_va,
                MapType::Framed,
                permission,
                handler,
                None,
                self.page_table.clone(),
                vma_type,
            ),
            None,
        );
    }

    /// Remove `VmArea` that starts with `start_vpn`
    #[allow(unused)]
    pub fn remove_area_with_start_vpn(&mut self, start_vpn: VirtPageNum) {
        stack_trace!();
        if let Some(area) = self.areas.get_unchecked_mut().get_mut(&start_vpn) {
            area.unmap_lazily();
            self.areas.get_unchecked_mut().remove(&start_vpn);
        }
    }

    /// Add the map area to memory set and map the map area(allocating physical
    /// frames)
    fn push(&mut self, mut vm_area: VmArea, data_offset: usize, data: Option<&[u8]>) {
        stack_trace!();
        vm_area.map();
        if let Some(data) = data {
            vm_area.copy_data_with_offset(data_offset, data);
        }
        self.areas.get_mut().insert(vm_area.start_vpn(), vm_area);
    }
    /// Only add the map area to memory set (without allocating physical frames)
    fn push_lazily(&self, vm_area: VmArea, _: Option<&[u8]>) {
        stack_trace!();
        self.areas
            .get_unchecked_mut()
            .insert(vm_area.start_vpn(), vm_area);
        // self.areas.push(vm_area);
    }

    /// Create a kernel space
    pub fn new_kernel() -> Self {
        stack_trace!();
        let mut memory_space = Self::new_bare();
        info!("[kernel] trampoline {:#x}", sigreturn_trampoline as usize);
        // // map trampoline
        // memory_space.map_trampoline();
        // map kernel sections
        info!(
            "[kernel].text [{:#x}, {:#x}) [{:#x}, {:#x})",
            stext as usize, strampoline as usize, etrampoline as usize, etext as usize
        );
        info!(
            "[kernel].text.trampoline [{:#x}, {:#x})",
            strampoline as usize, etrampoline as usize,
        );
        info!(
            "[kernel].rodata [{:#x}, {:#x})",
            srodata as usize, erodata as usize
        );
        info!(
            "[kernel].data [{:#x}, {:#x})",
            sdata as usize, edata as usize
        );
        info!(
            "[kernel].stack [{:#x}, {:#x})",
            sstack as usize, estack as usize
        );
        info!("[kernel].bss [{:#x}, {:#x})", sbss as usize, ebss as usize);
        info!(
            "[kernel]physical mem [{:#x}, {:#x})",
            ekernel as usize, MEMORY_END as usize
        );

        info!("[kernel]mapping .text section");
        memory_space.push(
            VmArea::new(
                (stext as usize).into(),
                (strampoline as usize).into(),
                MapType::Direct,
                MapPermission::R | MapPermission::X,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Elf,
            ),
            0,
            None,
        );
        memory_space.push(
            VmArea::new(
                (etrampoline as usize).into(),
                (etext as usize).into(),
                MapType::Direct,
                MapPermission::R | MapPermission::X,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Elf,
            ),
            0,
            None,
        );
        info!("[kernel]mapping .rodata section");
        memory_space.push(
            VmArea::new(
                (srodata as usize).into(),
                (erodata as usize).into(),
                MapType::Direct,
                MapPermission::R,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Elf,
            ),
            0,
            None,
        );
        info!("[kernel]mapping .data section");
        memory_space.push(
            VmArea::new(
                (sdata as usize).into(),
                (edata as usize).into(),
                MapType::Direct,
                MapPermission::R | MapPermission::W,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Elf,
            ),
            0,
            None,
        );
        // add stack section in `linker.ld`
        info!("[kernel]mapping .stack section");
        memory_space.push(
            VmArea::new(
                (sstack as usize).into(),
                (estack as usize).into(),
                MapType::Direct,
                MapPermission::R | MapPermission::W,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Elf,
            ),
            0,
            None,
        );
        info!("[kernel]mapping .bss section");
        memory_space.push(
            VmArea::new(
                (sbss as usize).into(),
                (ebss as usize).into(),
                MapType::Direct,
                MapPermission::R | MapPermission::W,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Elf,
            ),
            0,
            None,
        );
        info!("[kernel]mapping signal-return trampoline");
        memory_space.push(
            VmArea::new(
                (strampoline as usize).into(),
                (etrampoline as usize).into(),
                MapType::Direct,
                MapPermission::R | MapPermission::X | MapPermission::U,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Elf,
            ),
            0,
            None,
        );
        // info!("{:#x}", unsafe { *(strampoline as usize as *const usize) });
        info!("[kernel]mapping physical memory");
        memory_space.push(
            VmArea::new(
                (ekernel as usize).into(),
                MEMORY_END.into(),
                MapType::Direct,
                MapPermission::R | MapPermission::W,
                None,
                None,
                memory_space.page_table.clone(),
                VmAreaType::Physical,
            ),
            0,
            None,
        );
        info!("[kernel] mapping mmio registers");
        for pair in MMIO {
            info!("start va: {:#x}", pair.0);
            info!("end va: {:#x}", pair.0 + pair.1);
            info!("permission: {:?}", pair.2);
            memory_space.push(
                VmArea::new(
                    (pair.0 + (KERNEL_DIRECT_OFFSET << PAGE_SIZE_BITS)).into(),
                    (pair.0 + pair.1 + (KERNEL_DIRECT_OFFSET << PAGE_SIZE_BITS)).into(),
                    MapType::Direct,
                    pair.2,
                    None,
                    None,
                    memory_space.page_table.clone(),
                    VmAreaType::Mmio,
                ),
                0,
                None,
            );
        }
        info!("[kernel] new kernel finished");
        memory_space
    }

    /// Map the sections in the elf.
    /// Return the max end vpn and the first section's va.
    fn map_elf(
        &mut self,
        elf: &ElfFile,
        elf_file: Option<&Arc<dyn File>>,
        offset: VirtAddr,
    ) -> (VirtPageNum, VirtAddr) {
        stack_trace!();
        let elf_header = elf.header;
        let ph_count = elf_header.pt2.ph_count();

        let mut max_end_vpn = offset.floor();
        let mut header_va = 0;
        let mut has_found_header_va = false;
        info!("[map_elf]: entry point {:#x}", elf.header.pt2.entry_point());

        let page_cache = elf_file.map(|file| {
            file.metadata()
                .inner
                .lock()
                .inode
                .as_ref()
                .unwrap()
                .metadata()
                .inner
                .lock()
                .page_cache
                .as_ref()
                .unwrap()
                .clone()
        });

        // self.cow_pages.clear();
        // self.areas.get_mut().clear();
        // self.page_table.get_unchecked_mut().clear_user_space();

        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() != xmas_elf::program::Type::Load {
                continue;
            }
            let start_va: VirtAddr = (ph.virtual_addr() as usize + offset.0).into();
            let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize + offset.0).into();
            if !has_found_header_va {
                header_va = start_va.0;
                has_found_header_va = true;
            }
            let mut map_perm = MapPermission::U;
            let ph_flags = ph.flags();
            if ph_flags.is_read() {
                map_perm |= MapPermission::R;
            }
            if ph_flags.is_write() {
                map_perm |= MapPermission::W;
            }
            if ph_flags.is_execute() {
                map_perm |= MapPermission::X;
            }
            let mut vm_area = VmArea::new(
                start_va,
                end_va,
                MapType::Framed,
                map_perm,
                None,
                None,
                self.page_table.clone(),
                VmAreaType::Elf,
            );

            log::debug!(
                "[map_elf] [{:#x}, {:#x}], map_perm: {:?} start...",
                start_va.0,
                end_va.0,
                map_perm
            );

            max_end_vpn = vm_area.vpn_range.end();

            let map_offset = start_va.0 - start_va.floor().0 * PAGE_SIZE;

            log::debug!(
                "[map_elf] ph offset {:#x}, file size {:#x}, mem size {:#x}",
                ph.offset(),
                ph.file_size(),
                ph.mem_size()
            );
            if !map_perm.contains(MapPermission::W) && page_cache.is_some() {
                log::debug!(
                    "[map_elf] map shared page: [{:#x}, {:#x}]",
                    vm_area.start_vpn().0,
                    vm_area.end_vpn().0
                );
                let mut file_offset = ph.offset() as usize;
                for vpn in vm_area.vpn_range {
                    let page = page_cache
                        .as_ref()
                        .unwrap()
                        .get_page(file_offset, Some(map_perm))
                        .unwrap();
                    *page.permission.lock() = map_perm;
                    vm_area.map_one(vpn, Some(page));
                    file_offset += PAGE_SIZE;
                }
                self.push_lazily(vm_area, None);
            } else {
                self.push(
                    vm_area,
                    map_offset,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                );
            }

            info!(
                "[map_elf] [{:#x}, {:#x}], map_perm: {:?}",
                start_va.0, end_va.0, map_perm
            );
        }

        (max_end_vpn, header_va.into())
    }

    /// Include sections in elf and TrapContext and user stack,
    /// also returns user_sp and entry point.
    /// TODO: resolve elf file lazily
    pub fn from_elf(
        elf_data: &[u8],
        elf_file: Option<&Arc<dyn File>>,
    ) -> (Self, usize, usize, Vec<AuxHeader>) {
        stack_trace!();
        let mut memory_space = Self::new_from_global();

        // map program headers of elf, with U flag
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        assert_eq!(
            elf_header.pt1.magic,
            [0x7f, 0x45, 0x4c, 0x46],
            "invalid elf!"
        );
        let mut entry_point = elf_header.pt2.entry_point() as usize;
        let ph_entry_size = elf_header.pt2.ph_entry_size() as usize;
        let ph_count = elf_header.pt2.ph_count() as usize;

        let mut auxv = generate_early_auxv(ph_entry_size, ph_count, entry_point);
        if let Some(interp_entry_point) = memory_space.load_dl_interp_if_needed(&elf) {
            auxv.push(AuxHeader::new(AT_BASE, DL_INTERP_OFFSET));
            entry_point = interp_entry_point;
        } else {
            auxv.push(AuxHeader::new(AT_BASE, 0));
        }

        let (max_end_vpn, header_va) = memory_space.map_elf(&elf, elf_file, 0.into());

        let ph_head_addr = header_va.0 + elf.header.pt2.ph_offset() as usize;
        debug!("[from_elf] AT_PHDR  ph_head_addr is {:x} ", ph_head_addr);
        auxv.push(AuxHeader::new(AT_PHDR, ph_head_addr));

        // map user stack with U flags
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_bottom: usize = max_end_va.into();
        // guard page
        user_stack_bottom += PAGE_SIZE;

        // We will add the ustack to memory set later by `Thread` itself
        // Now we add heap section
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;

        let ustack_vma = VmArea::new(
            user_stack_bottom.into(),
            user_stack_top.into(),
            MapType::Framed,
            MapPermission::U | MapPermission::R | MapPermission::W,
            Some(Arc::new(UStackPageFaultHandler {})),
            None,
            memory_space.page_table.clone(),
            VmAreaType::Stack,
        );
        memory_space.push_lazily(ustack_vma, None);
        log::info!(
            "[from_elf] map ustack: {:#x}, {:#x}",
            user_stack_bottom,
            user_stack_top,
        );

        // guard page
        let heap_start_va = user_stack_top + PAGE_SIZE;
        let heap_end_va = heap_start_va;
        let map_perm = MapPermission::U | MapPermission::R | MapPermission::W;
        let heap_vma = VmArea::new(
            heap_start_va.into(),
            heap_end_va.into(),
            MapType::Framed,
            map_perm,
            Some(Arc::new(SBrkPageFaultHandler {})),
            None,
            memory_space.page_table.clone(),
            VmAreaType::Brk,
        );
        memory_space.push(heap_vma, 0, None);
        memory_space.heap_range = Some(HeapRange::new(heap_start_va.into(), heap_end_va.into()));
        log::info!(
            "[from_elf] map heap: {:#x}, {:#x}",
            heap_start_va,
            heap_end_va
        );

        (memory_space, user_stack_top, entry_point, auxv)
    }

    /// Check whether the elf file is dynamic linked and
    /// if so, load the dl interpreter.
    /// Return the interpreter's entry point(at the base of DL_INTERP_OFFSET) if
    /// so.
    fn load_dl_interp_if_needed(&mut self, elf: &ElfFile) -> Option<usize> {
        stack_trace!();
        let elf_header = elf.header;
        let ph_count = elf_header.pt2.ph_count();

        let mut is_dl = false;
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() == xmas_elf::program::Type::Interp {
                is_dl = true;
                break;
            }
        }

        if is_dl {
            log::info!("[load_dl] encounter a dl elf");
            let section = elf.find_section_by_name(".interp").unwrap();
            let mut interp = String::from_utf8(section.raw_data(&elf).to_vec()).unwrap();
            interp = interp.strip_suffix("\0").unwrap_or(&interp).to_string();
            log::info!("[load_dl] interp {}", interp);

            let mut interps: Vec<String> = vec![interp.clone()];

            log::info!("interp {}", interp);

            if interp.eq("/lib/ld-musl-riscv64-sf.so.1") || interp.eq("/lib/ld-musl-riscv64.so.1") {
                // interp = "/lib/libc.so".to_string();
                interps.push("/libc.so".to_string());
                interps.push("/lib/libc.so".to_string());
            }

            let mut interp_inode = None;
            for interp in interps {
                if let Some(inode) = resolve_path(AT_FDCWD, &interp, OpenFlags::RDONLY).ok() {
                    interp_inode = Some(inode);
                    break;
                }
            }
            let interp_inode = interp_inode.unwrap();
            let interp_file = interp_inode.open(interp_inode.clone()).ok().unwrap();
            let mut interp_elf_data = Vec::new();
            interp_file
                .read_all_from_start(&mut interp_elf_data)
                .ok()
                .unwrap();
            let interp_elf = xmas_elf::ElfFile::new(&interp_elf_data).unwrap();
            self.map_elf(&interp_elf, Some(&interp_file), DL_INTERP_OFFSET.into());

            Some(interp_elf.header.pt2.entry_point() as usize + DL_INTERP_OFFSET)
        } else {
            debug!("[load_dl] encounter a static elf");
            None
        }
    }

    /// Clone a same `MemorySpace`
    #[allow(unused)]
    pub fn from_existed_user(user_space: &Self) -> Self {
        stack_trace!();
        // let mut memory_space = Self::new_bare();
        let mut memory_space = Self::new_from_global();
        // copy data sections/trap_context/user_stack
        for (_, area) in user_space.areas.get_unchecked_mut().iter() {
            let mut new_area = VmArea::from_another(area, memory_space.page_table.clone());
            // memory_space.push(new_area, None);
            // copy data from another space
            for vpn in area.vpn_range {
                if let Some(ppn) = user_space.translate(vpn) {
                    let src_ppn = ppn.ppn();
                    let dst_ppn = new_area.map_one(vpn, None);
                    dst_ppn.bytes_array().copy_from_slice(src_ppn.bytes_array());
                }
                // let src_ppn = user_space.translate(vpn).unwrap().ppn();
                // let dst_ppn = memory_space.translate(vpn).unwrap().ppn();
                // dst_ppn
                //     .get_bytes_array()
                //     .copy_from_slice(src_ppn.get_bytes_array());
            }
            memory_space.push_lazily(new_area, None);
        }
        memory_space.heap_range = user_space.heap_range;
        memory_space
    }
    /// Clone a same `MemorySpace`
    pub fn from_existed_user_lazily(user_space: &mut Self) -> Self {
        stack_trace!();
        // TODO: optimize: no need to new a CowPageManager
        let mut memory_space = Self::new_from_global();
        // SAFETY: the process inner has been locked when invoking this function
        memory_space.cow_pages =
            CowPageManager::from_another(&user_space.cow_pages, memory_space.page_table.clone());

        let new_pagetable = memory_space.page_table.get_unchecked_mut();

        // copy data sections/trap_context/user_stack
        for (_, area) in user_space.areas.get_unchecked_mut().iter() {
            let new_area = VmArea::from_another(area, memory_space.page_table.clone());
            info!(
                "[from_existed_user_lazily] area range [{:#x}, {:#x}), map perm {:?}",
                new_area.start_vpn().0,
                new_area.end_vpn().0,
                new_area.map_perm,
            );
            // copy data from another space
            for vpn in area.vpn_range {
                // SAFETY: we've locked the process inner before calling this function
                if let Some(ph_frame) = area.data_frames.get_unchecked_mut().0.get(&vpn) {
                    // If there is related physcial frame, then we let the child and father share
                    // it.
                    let pte = user_space
                        .page_table
                        .get_unchecked_mut()
                        .find_pte(vpn)
                        .unwrap();
                    trace!(
                        "change vpn {:#x} to cow page, ppn {:#x}, pte flags {:?}",
                        vpn.0,
                        ph_frame.data_frame.ppn.0,
                        pte.flags()
                    );

                    let (pte_flags, ppn) = match area.vma_type {
                        VmAreaType::Shm => {
                            // If shared memory,
                            // then we don't need to modify the pte flags,
                            // i.e. no copy-on-write.
                            info!("[from_existed_user_lazily] vma type {:?}", area.vma_type);
                            new_area
                                .data_frames
                                .get_unchecked_mut()
                                .0
                                .insert(vpn, ph_frame.clone());
                            (pte.flags(), ph_frame.data_frame.ppn)
                        }
                        _ => {
                            // Else,
                            // copy-on-write
                            let mut new_flags = pte.flags() | PTEFlags::COW;
                            new_flags.remove(PTEFlags::W);
                            pte.set_flags(new_flags);
                            debug_assert!(pte.flags().contains(PTEFlags::COW));
                            debug_assert!(!pte.flags().contains(PTEFlags::W));
                            user_space
                                .cow_pages
                                .page_mgr
                                .get_unchecked_mut()
                                .0
                                .insert(vpn, ph_frame.clone());
                            memory_space
                                .cow_pages
                                .page_mgr
                                .get_unchecked_mut()
                                .0
                                .insert(vpn, ph_frame.clone());
                            let ppn = ph_frame.data_frame.ppn;
                            area.data_frames.get_unchecked_mut().0.remove(&vpn);
                            (new_flags, ppn)
                        }
                    };

                    new_pagetable.map(vpn, ppn, pte_flags);
                } else {
                    // trace!("no ppn for vpn {:#x}", vpn.0);
                }
            }
            memory_space.push_lazily(new_area, None);
        }
        memory_space.heap_range = user_space.heap_range;
        // user_space.activate();
        // new_pagetable.activate();

        memory_space
    }
    /// Refresh TLB with `sfence.vma`
    pub fn activate(&self) {
        stack_trace!();
        self.page_table.get_unchecked_mut().activate()
    }
    /// Translate throuth pagetable
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        stack_trace!();
        unsafe { (*self.page_table.get()).translate(vpn) }
    }

    /// Remove all `VmArea`
    #[allow(unused)]
    pub fn recycle_data_pages(&mut self) {
        stack_trace!();
        //*self = Self::new_bare();
        self.areas.get_unchecked_mut().clear();
    }

    /// Allocate an unused area by specific start va.
    /// Note that length is counted by byte.
    pub fn allocate_spec_area(
        &mut self,
        length: usize,
        map_permission: MapPermission,
        start_va: VirtAddr,
        vma_type: VmAreaType,
    ) -> GeneralRet<Option<VmArea>> {
        stack_trace!();
        if length == 0 {
            return Ok(None);
        }
        let length_rounded = (length - 1 + PAGE_SIZE) / PAGE_SIZE * PAGE_SIZE;
        let end_va: VirtAddr = (start_va.0 + length_rounded).into();
        debug!(
            "[allocate_spec_area] start va {:#x}, end va {:#x}",
            start_va.0, end_va.0
        );
        if start_va.0 % PAGE_SIZE != 0 {
            return Err(SyscallErr::EINVAL);
        }
        // TODO: just sanity check, should find a safer way
        // TODO: check more carefully
        if let Some(clipped_vma) =
            self.clip_vm_areas_overlapping(VPNRange::new(start_va.floor(), end_va.ceil()))?
        {
            let backup_file = match clipped_vma.backup_file.as_ref() {
                Some(bak) => {
                    let new_offset =
                        bak.offset + start_va.0 - VirtAddr::from(clipped_vma.start_vpn()).0;
                    log::debug!(
                        "[allocate_spec_area] new area offset {:#x}, old area offset {:#x}",
                        new_offset,
                        bak.offset
                    );
                    Some(BackupFile::new(new_offset, bak.file.clone()))
                }
                None => None,
            };
            Ok(Some(VmArea::new(
                start_va,
                end_va,
                MapType::Framed,
                map_permission,
                clipped_vma.handler.clone(),
                backup_file,
                self.page_table.clone(),
                vma_type,
            )))
        } else {
            Ok(Some(VmArea::new(
                start_va,
                end_va,
                MapType::Framed,
                map_permission,
                None,
                None,
                self.page_table.clone(),
                vma_type,
            )))
        }
        // if self.find_vm_area_by_vpn(start_va.floor()).is_some() {
        //     warn!("[allocate_spec_area] conflicted vm area!");
        //     return None;
        // }
    }

    /// Allocate an unused area(mostly for mmap).
    /// Note that length is counted by byte.
    pub fn allocate_area(
        &self,
        length: usize,
        map_permission: MapPermission,
        vma_type: VmAreaType,
    ) -> Option<VmArea> {
        stack_trace!();
        if length == 0 {
            return None;
        }
        let mut last_start = MMAP_TOP;
        // traverse reversely
        let length_rounded = (length - 1 + PAGE_SIZE) / PAGE_SIZE * PAGE_SIZE;
        for (start_vpn, vma) in self.areas.get_unchecked_mut().iter().rev() {
            log::trace!(
                "key start {:#x}, start {:#x}, end {:#x}",
                start_vpn.0,
                vma.start_vpn().0,
                vma.end_vpn().0
            );
            let curr_end = vma.end_vpn().0 * PAGE_SIZE;
            if last_start - curr_end >= length_rounded {
                let new_start = last_start - length_rounded;
                log::debug!("[allocate_area] [{:#x}, {:#x}]", new_start, last_start);
                return Some(VmArea::new(
                    new_start.into(),
                    last_start.into(),
                    MapType::Framed,
                    map_permission,
                    None,
                    None,
                    self.page_table.clone(),
                    vma_type,
                ));
            }
            last_start = vma.start_vpn().0 * PAGE_SIZE;
        }
        error!("[allocate area] cannot find any unused vm area!!");
        None
    }

    /// Check whether the given vpn range conflicts with other vma.
    /// Note that the start_vpn must have been in memory set.
    pub fn check_vpn_range_conflict(&self, start_vpn: VirtPageNum, end_vpn: VirtPageNum) -> bool {
        stack_trace!();
        for vma in self.areas.get_unchecked_mut().iter() {
            if *vma.0 == start_vpn {
                continue;
            }
            if vma.1.end_vpn() > start_vpn && vma.1.start_vpn() < end_vpn {
                debug!(
                    "conflict vpn range: input vpnr: [{:#x}, {:#x}], old vpnr: [{:#x}, {:#x}]",
                    start_vpn.0,
                    end_vpn.0,
                    vma.1.start_vpn().0,
                    vma.1.end_vpn().0
                );
                return true;
            }
        }
        return false;
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
/// map type for memory set: identical or framed
pub enum MapType {
    /// vpn == ppn
    #[allow(unused)]
    Identical,
    /// vpn == ppn + offset
    Direct,
    ///
    Framed,
}

#[allow(unused)]
/// Check PageTable running correctly
pub fn remap_test() {
    stack_trace!();
    // todo!();
    info!("remap_test start...");
    let kernel_space = unsafe { KERNEL_SPACE.as_ref().unwrap() };
    // let mid_text: VirtAddr = ((stext as usize + etext as usize) / 2).into();
    let mid_text: VirtAddr = (stext as usize + (etext as usize - stext as usize) / 2).into();
    let mid_rodata: VirtAddr =
        (srodata as usize + (erodata as usize - srodata as usize) / 2).into();
    let mid_data: VirtAddr = (sdata as usize + (edata as usize - sdata as usize) / 2).into();
    debug!(
        "mid text {:#x}, mid rodata {:#x}, mid data {:#x}",
        mid_text.0, mid_rodata.0, mid_data.0
    );
    unsafe {
        assert!(!(*kernel_space.page_table.get())
            .translate(mid_text.floor())
            .unwrap()
            .writable(),);
        assert!(!(*kernel_space.page_table.get())
            .translate(mid_rodata.floor())
            .unwrap()
            .writable(),);
        assert!(!(*kernel_space.page_table.get())
            .translate(mid_data.floor())
            .unwrap()
            .executable(),);
    }
    info!("remap_test passed!");
}

/// Handle different kinds of page fault
pub async fn handle_page_fault(va: VirtAddr, scause: Scause) -> GeneralRet<()> {
    stack_trace!();
    if let Some(handler) = current_process().inner_handler(|proc| {
        let (handler, vma) = proc.memory_space.page_fault_handler(va, scause)?;
        if !handler.handle_page_fault(va, &proc.memory_space, vma)? {
            Ok(Some(handler))
        } else {
            Ok(None)
        }
    })? {
        debug!("handle pagefault asynchronously, va: {:#x}", va.0);
        handler
            .handle_page_fault_async(va, current_process(), scause)
            .await
    } else {
        Ok(())
    }
}