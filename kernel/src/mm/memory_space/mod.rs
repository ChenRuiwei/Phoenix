use alloc::{
    string::{String, ToString},
    sync::{Arc, Weak},
    vec,
    vec::Vec,
};
use core::{
    cell::SyncUnsafeCell,
    cmp,
    ops::{Range, RangeBounds},
};

use arch::memory::sfence_vma_vaddr;
use async_utils::block_on;
use config::{
    mm::{
        is_aligned_to_page, round_down_to_page, DL_INTERP_OFFSET, MMAP_PRE_ALLOC_PAGES, PAGE_SIZE,
        USER_ELF_PRE_ALLOC_PAGE_CNT, U_SEG_FILE_BEG, U_SEG_FILE_END, U_SEG_HEAP_BEG,
        U_SEG_HEAP_END, U_SEG_SHARE_BEG, U_SEG_SHARE_END, U_SEG_STACK_BEG, U_SEG_STACK_END,
    },
    process::USER_STACK_PRE_ALLOC_SIZE,
};
use memory::{pte::PTEFlags, PageTable, PhysAddr, VirtAddr, VirtPageNum};
use page::Page;
use range_map::RangeMap;
use systype::{SysError, SysResult};
use vfs_core::{Dentry, File};
use xmas_elf::ElfFile;

use self::vm_area::VmArea;
use super::{kernel_page_table, PageFaultAccessType};
use crate::{
    mm::memory_space::vm_area::{MapPerm, VmAreaType},
    processor::{env::SumGuard, hart::current_task_ref},
    syscall::MmapFlags,
    task::{
        aux::{generate_early_auxv, AuxHeader, AT_BASE, AT_NULL, AT_PHDR, AT_RANDOM},
        Task,
    },
};

pub mod vm_area;

/// Virtual memory space for user.
pub struct MemorySpace {
    // NOTE: The reason why `page_table` and `areas` are `SyncUnsafeCell` is because they both
    // represent memory region, it is likely to modify the two both.
    /// Page table of this memory space.
    page_table: SyncUnsafeCell<PageTable>,
    /// Map of `VmArea`s in this memory space.
    /// NOTE: stores range that is lazy allocated
    areas: SyncUnsafeCell<RangeMap<VirtAddr, VmArea>>,
}

impl MemorySpace {
    /// Create an empty `MemorySpace`
    pub fn new() -> Self {
        Self {
            page_table: SyncUnsafeCell::new(PageTable::new()),
            areas: SyncUnsafeCell::new(RangeMap::new()),
        }
    }

    /// Create a new user memory space that inherits kernel page table.
    pub fn new_user() -> Self {
        Self {
            page_table: SyncUnsafeCell::new(PageTable::from_kernel(kernel_page_table())),
            areas: SyncUnsafeCell::new(RangeMap::new()),
        }
    }

    pub fn areas(&self) -> &RangeMap<VirtAddr, VmArea> {
        unsafe { &*self.areas.get() }
    }

    pub fn areas_mut(&self) -> &mut RangeMap<VirtAddr, VmArea> {
        unsafe { &mut *self.areas.get() }
    }

    pub fn page_table(&self) -> &PageTable {
        unsafe { &*self.page_table.get() }
    }

    pub fn page_table_mut(&self) -> &mut PageTable {
        unsafe { &mut *self.page_table.get() }
    }

    /// Map the sections in the elf.
    ///
    /// Return the max end vpn and the first section's va.
    pub fn map_elf(
        &mut self,
        elf_file: Arc<dyn File>,
        elf: &ElfFile,
        offset: VirtAddr,
    ) -> (VirtPageNum, VirtAddr) {
        let elf_header = elf.header;
        let ph_count = elf_header.pt2.ph_count();

        let mut max_end_vpn = offset.floor();
        let mut header_va = 0;
        let mut has_found_header_va = false;
        log::info!("[map_elf]: entry point {:#x}", elf.header.pt2.entry_point());

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
            let mut map_perm = MapPerm::U;
            let ph_flags = ph.flags();
            if ph_flags.is_read() {
                map_perm |= MapPerm::R;
            }
            if ph_flags.is_write() {
                map_perm |= MapPerm::W;
            }
            if ph_flags.is_execute() {
                map_perm |= MapPerm::X;
            }
            let mut vm_area = VmArea::new(start_va..end_va, map_perm, VmAreaType::Elf);

            log::debug!("[map_elf] [{start_va:#x}, {end_va:#x}], map_perm: {map_perm:?} start...",);

            max_end_vpn = vm_area.end_vpn();

            let map_offset = start_va - start_va.round_down();

            log::debug!(
                "[map_elf] ph offset {:#x}, file size {:#x}, mem size {:#x}",
                ph.offset(),
                ph.file_size(),
                ph.mem_size()
            );

            if ph.file_size() == ph.mem_size() && is_aligned_to_page(ph.offset() as usize) {
                assert!(!map_perm.contains(MapPerm::W));
                // NOTE: only add cow flag in elf page newly mapped.
                // FIXME: mprotect is not checked yet
                // WARN: the underlying elf file page cache may be edited, may cause unknown
                // behavior
                let mut pre_alloc_page_cnt = 0;
                for vpn in vm_area.range_vpn() {
                    let start_offset = ph.offset() as usize;
                    let offset = start_offset + (vpn - vm_area.start_vpn()) * PAGE_SIZE;
                    let offset_aligned = round_down_to_page(offset);
                    if let Some(page) =
                        block_on(async { elf_file.get_page_at(offset_aligned).await }).unwrap()
                    {
                        if pre_alloc_page_cnt < USER_ELF_PRE_ALLOC_PAGE_CNT {
                            let new_page = Page::new();
                            // WARN: area outer than region may should be set to zero
                            new_page.copy_from_slice(page.bytes_array());
                            self.page_table_mut()
                                .map(vpn, new_page.ppn(), map_perm.into());
                            vm_area.pages.insert(vpn, new_page);
                            unsafe { sfence_vma_vaddr(vpn.to_vaddr().into()) };
                        } else {
                            let (pte_flags, ppn) = {
                                let mut new_flags: PTEFlags = map_perm.into();
                                new_flags |= PTEFlags::COW;
                                new_flags.remove(PTEFlags::W);
                                (new_flags, page.ppn())
                            };
                            self.page_table_mut().map(vpn, ppn, pte_flags);
                            vm_area.pages.insert(vpn, page);
                            unsafe { sfence_vma_vaddr(vpn.to_vaddr().into()) };
                        }
                        pre_alloc_page_cnt += 1;
                    } else {
                        break;
                    }
                }
                self.push_vma_lazily(vm_area);
                log::info!("[map_elf] [{start_va:#x}, {end_va:#x}], map_perm: {map_perm:?}",);
            } else {
                self.push_vma_with_data(
                    vm_area,
                    map_offset,
                    &elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize],
                );
            }
        }

        (max_end_vpn, header_va.into())
    }

    pub fn parse_and_map_elf(
        &mut self,
        elf_file: Arc<dyn File>,
        elf_data: &[u8],
    ) -> (usize, Vec<AuxHeader>) {
        const ELF_MAGIC: [u8; 4] = [0x7f, 0x45, 0x4c, 0x46];

        // map program headers of elf, with U flag
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        assert_eq!(elf_header.pt1.magic, ELF_MAGIC, "invalid elf!");
        let entry = elf_header.pt2.entry_point() as usize;
        let ph_entry_size = elf_header.pt2.ph_entry_size() as usize;
        let ph_count = elf_header.pt2.ph_count() as usize;

        let mut auxv = generate_early_auxv(ph_entry_size, ph_count, entry);

        auxv.push(AuxHeader::new(AT_BASE, 0));

        let (_max_end_vpn, header_va) = self.map_elf(elf_file, &elf, 0.into());

        let ph_head_addr = header_va.0 + elf.header.pt2.ph_offset() as usize;
        auxv.push(AuxHeader::new(AT_RANDOM, ph_head_addr));
        log::debug!("[parse_and_map_elf] AT_PHDR  ph_head_addr is {ph_head_addr:x}",);
        auxv.push(AuxHeader::new(AT_PHDR, ph_head_addr));

        (entry, auxv)
    }

    /// Check whether the elf file is dynamic linked and if so, load the dl
    /// interpreter.
    ///
    /// Return the interpreter's entry point(at the base of DL_INTERP_OFFSET) if
    /// so.
    pub fn load_dl_interp_if_needed(&mut self, elf: &ElfFile) -> Option<usize> {
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
                interps.clear();
                interps.push("/lib/musl/libc.so".to_string());
            } else if interp.eq("/lib/ld-linux-riscv64-lp64d.so.1") {
                interps.clear();
                interps.push("/lib/glibc/ld-2.31.so".to_string());
            }
            let mut interp_dentry: SysResult<Arc<dyn Dentry>> = Err(SysError::ENOENT);
            for interp in interps.into_iter() {
                if let Ok(dentry) = current_task_ref().resolve_path(&interp) {
                    interp_dentry = Ok(dentry);
                    break;
                }
            }
            let interp_dentry: Arc<dyn Dentry> = interp_dentry.unwrap();
            let interp_file = interp_dentry.open().ok().unwrap();
            let interp_elf_data = block_on(async { interp_file.read_all().await }).ok()?;
            let interp_elf = xmas_elf::ElfFile::new(&interp_elf_data).unwrap();
            self.map_elf(interp_file, &interp_elf, DL_INTERP_OFFSET.into());

            Some(interp_elf.header.pt2.entry_point() as usize + DL_INTERP_OFFSET)
        } else {
            log::debug!("[load_dl] encounter a static elf");
            None
        }
    }

    /// Attach given `pages` to the MemorySpace. If pages is not given, it will
    /// create pages according to the `size` and map them to the MemorySpace.
    /// if `shmaddr` is set to `0`, it will chooses a suitable page-aligned
    /// address to attach.
    ///
    /// `size` and `shmaddr` need to be page-aligned
    pub fn attach_shm(
        &mut self,
        size: usize,
        shmaddr: VirtAddr,
        map_perm: MapPerm,
        pages: &mut Vec<Weak<Page>>,
    ) -> VirtAddr {
        let mut ret_addr = shmaddr;
        let mut vm_area = if shmaddr == 0.into() {
            const SHARED_RANGE: Range<VirtAddr> =
                VirtAddr::from_usize_range(U_SEG_SHARE_BEG..U_SEG_SHARE_END);
            let range = self
                .areas()
                .find_free_range(SHARED_RANGE, size)
                .expect("no free shared area");
            ret_addr = range.start;
            VmArea::new(range, map_perm, VmAreaType::Shm)
        } else {
            log::info!("[attach_shm] user defined addr");
            let shm_end = shmaddr + size;
            VmArea::new(shmaddr..shm_end, map_perm, VmAreaType::Shm)
        };
        if pages.is_empty() {
            for vpn in vm_area.range_vpn() {
                let page = Page::new();
                self.page_table_mut().map(vpn, page.ppn(), map_perm.into());
                pages.push(Arc::downgrade(&page));
                vm_area.pages.insert(vpn, page);
            }
        } else {
            debug_assert!(pages.len() == vm_area.range_vpn().end - vm_area.range_vpn().start);
            let mut pages = pages.iter();
            for vpn in vm_area.range_vpn() {
                let page = pages.next().unwrap().upgrade().unwrap();
                self.page_table_mut().map(vpn, page.ppn(), map_perm.into());
                vm_area.pages.insert(vpn, page.clone());
            }
        }
        self.push_vma_lazily(vm_area);
        return ret_addr;
    }

    /// `shmaddr` must be the return value of shmget (i.e. `shmaddr` is page
    /// aligned and in the beginning of the vm_area with type Shm). The
    /// check should be done at the caller who call `detach_shm`
    pub fn detach_shm(&mut self, shmaddr: VirtAddr) {
        let mut range_to_remove = None;
        if let Some((range, vm_area)) = self
            .areas()
            .iter()
            .find(|(range, _)| range.start == shmaddr)
        {
            if vm_area.vma_type != VmAreaType::Shm {
                panic!("[detach_shm] 'vm_area.vma_type != VmAreaType::Shm' this won't happen");
            }
            log::info!("[detach_shm] try to remove {:?}", range);
            range_to_remove = Some(range);
            for vpn in vm_area.range_vpn() {
                self.page_table_mut().unmap(vpn);
            }
        } else {
            panic!("[detach_shm] this won't happen");
        }
        if let Some(range) = range_to_remove {
            self.areas_mut().force_remove_one(range);
        } else {
            panic!("[detach_shm] range_to_remove is None! This should never happen");
        }
    }

    /// Alloc stack and map it in the page table.
    ///
    /// Return the address of the stack top, which is aligned to 16 bytes.
    ///
    /// The stack has a range of [sp - size, sp].
    pub fn alloc_stack_lazily(&mut self, size: usize) -> VirtAddr {
        const STACK_RANGE: Range<VirtAddr> =
            VirtAddr::from_usize_range(U_SEG_STACK_BEG..U_SEG_STACK_END);

        let range = self
            .areas()
            .find_free_range(STACK_RANGE, size)
            .expect("too many stack!");

        // align to 16 bytes
        let sp_init = VirtAddr::from((range.end.bits() - 1) & !0xf);
        log::debug!("[MemorySpace::alloc_stack] stack: {range:x?}, sp_init: {sp_init:x?}");

        let mut vm_area = VmArea::new(range.clone(), MapPerm::URW, VmAreaType::Stack);
        vm_area.map_range(
            self.page_table_mut(),
            range.end - USER_STACK_PRE_ALLOC_SIZE..range.end,
        );
        self.push_vma_lazily(vm_area);
        sp_init
    }

    /// Alloc heap lazily.
    pub fn alloc_heap_lazily(&mut self) {
        const HEAP_RANGE: Range<VirtAddr> =
            VirtAddr::from_usize_range(U_SEG_HEAP_BEG..U_SEG_HEAP_END);

        const INIT_SIZE: usize = PAGE_SIZE;
        let range = VirtAddr::from_usize_range(U_SEG_HEAP_BEG..U_SEG_HEAP_BEG + INIT_SIZE);

        let vm_area = VmArea::new(range, MapPerm::URW, VmAreaType::Heap);
        self.push_vma_lazily(vm_area);
    }

    pub fn get_heap_break(&self) -> VirtAddr {
        // HACK: directly get U_SEG_HEAP_BEG instead？
        let (range, _) = self
            .areas()
            .iter()
            .find(|(_, vma)| vma.vma_type == VmAreaType::Heap)
            .unwrap();
        range.end
    }

    /// NOTE: The actual Linux system call returns the new program break on
    /// success. On failure, the system call returns the current break.
    pub fn reset_heap_break(&mut self, new_brk: VirtAddr) -> VirtAddr {
        let (range, _vma) = self
            .areas_mut()
            .iter_mut()
            .find(|(_, vma)| vma.vma_type == VmAreaType::Heap)
            .unwrap();
        log::debug!("[MemorySpace::reset_heap_break] heap range: {range:?}, new_brk: {new_brk:?}");
        let result = if new_brk > range.end {
            let ret = self.areas_mut().extend_back(range.start..new_brk);
            if ret.is_ok() {
                let (range_va, vm_area) = self.areas_mut().get_key_value_mut(range.start).unwrap();
                vm_area.set_range_va(range_va);
            }
            ret
        } else if new_brk < range.end {
            let ret = self.areas_mut().reduce_back(range.start, new_brk);
            if ret.is_ok() {
                let (range_va, _) = self.areas_mut().get_key_value(range.start).unwrap();
                let vma = self.areas_mut().force_remove_one(range_va.clone());
                let (left, middle, right) = vma.split(range_va);
                debug_assert!(left.is_none());
                debug_assert!(middle.is_some());
                debug_assert!(right.is_some());
                let mut right_vma = right.unwrap();
                right_vma.unmap(self.page_table_mut());
                self.push_vma_lazily(middle.unwrap());
            }
            ret
        } else {
            Ok(())
        };
        match result {
            Ok(_) => new_brk,
            Err(_) => range.end,
        }
    }

    /// Clone a same `MemorySpace` lazily.
    pub fn from_user_lazily(user_space: &mut Self) -> Self {
        let mut memory_space = Self::new_user();
        for (range, area) in user_space.areas().iter() {
            log::debug!("[MemorySpace::from_user_lazily] cloning {area:?}");
            let mut new_area = area.clone();
            debug_assert_eq!(range, new_area.range_va());
            for vpn in area.range_vpn() {
                if let Some(page) = area.pages.get(&vpn) {
                    let pte = user_space.page_table_mut().find_leaf_pte(vpn).unwrap();
                    let (pte_flags, ppn) = match area.vma_type {
                        VmAreaType::Shm => {
                            // If shared memory,
                            // then we don't need to modify the pte flags,
                            // i.e. no copy-on-write.
                            log::info!("[from_user_lazily] clone Shared Memory");
                            new_area.pages.insert(vpn, page.clone());
                            (pte.flags(), page.ppn())
                        }
                        _ => {
                            // copy on write
                            // TODO: MmapFlags::MAP_SHARED
                            let mut new_flags = pte.flags() | PTEFlags::COW;
                            new_flags.remove(PTEFlags::W);
                            pte.set_flags(new_flags);
                            (new_flags, page.ppn())
                        }
                    };
                    memory_space.page_table_mut().map(vpn, ppn, pte_flags);
                } else {
                    // lazy allocated area
                }
            }
            memory_space.push_vma_lazily(new_area);
        }
        memory_space
    }

    /// Push `VmArea` into `MemorySpace` and map it in page table.
    pub fn push_vma(&mut self, mut vma: VmArea) {
        vma.map(self.page_table_mut());
        self.areas_mut().try_insert(vma.range_va(), vma).unwrap();
    }

    /// Push `VmArea` into `MemorySpace` without mapping it in page table.
    pub fn push_vma_lazily(&mut self, vma: VmArea) {
        self.areas_mut().try_insert(vma.range_va(), vma).unwrap();
    }

    /// Push `VmArea` into `MemorySpace` and map it in page table, also copy
    /// `data` at `offset` of `vma`.
    pub fn push_vma_with_data(&mut self, mut vma: VmArea, offset: usize, data: &[u8]) {
        vma.map(self.page_table_mut());
        vma.fill_zero();
        vma.copy_data_with_offset(self.page_table_mut(), offset, data);
        self.areas_mut().try_insert(vma.range_va(), vma).unwrap();
    }

    pub fn alloc_mmap_shared_anonymous(
        &mut self,
        addr: VirtAddr,
        length: usize,
        perm: MapPerm,
        flags: MmapFlags,
    ) -> SysResult<VirtAddr> {
        const SHARED_RANGE: Range<VirtAddr> =
            VirtAddr::from_usize_range(U_SEG_SHARE_BEG..U_SEG_SHARE_END);
        let range = if flags.contains(MmapFlags::MAP_FIXED) {
            addr..addr + length
        } else {
            self.areas_mut()
                .find_free_range(SHARED_RANGE, length)
                .expect("shared range is full")
        };
        let start = range.start;
        let vma = VmArea::new(range, perm, VmAreaType::Shm);
        self.push_vma(vma);
        // self.areas_mut().try_insert(vma.range_va(), vma).unwrap();
        Ok(start)
    }

    pub fn alloc_mmap_anonymous(
        &mut self,
        addr: VirtAddr,
        length: usize,
        perm: MapPerm,
        flags: MmapFlags,
    ) -> SysResult<VirtAddr> {
        const MMAP_RANGE: Range<VirtAddr> =
            VirtAddr::from_usize_range(U_SEG_FILE_BEG..U_SEG_FILE_END);
        let range = if flags.contains(MmapFlags::MAP_FIXED) {
            addr..addr + length
        } else {
            self.areas_mut()
                .find_free_range(MMAP_RANGE, length)
                .expect("mmap range is full")
        };
        let start = range.start;
        let vma = VmArea::new_mmap(range, perm, flags, None, 0);
        self.areas_mut().try_insert(vma.range_va(), vma).unwrap();
        Ok(start)
    }

    // NOTE: can not alloc all pages from `PageCache`, otherwise lmbench
    // lat_pagefault will test page fault time as zero.
    pub fn alloc_mmap_area_lazily(
        &mut self,
        addr: VirtAddr,
        length: usize,
        perm: MapPerm,
        flags: MmapFlags,
        file: Arc<dyn File>,
        offset: usize,
    ) -> SysResult<VirtAddr> {
        debug_assert!(is_aligned_to_page(offset));

        const MMAP_RANGE: Range<VirtAddr> =
            VirtAddr::from_usize_range(U_SEG_FILE_BEG..U_SEG_FILE_END);

        let range = if flags.contains(MmapFlags::MAP_FIXED) {
            addr..addr + length
        } else {
            self.areas_mut()
                .find_free_range(MMAP_RANGE, length)
                .expect("mmap range is full")
        };
        let start = range.start;

        let page_table = self.page_table_mut();
        let inode = file.inode();
        let mut vma = VmArea::new_mmap(range, perm, flags, Some(file.clone()), offset);
        let mut range_vpn = vma.range_vpn();
        let length = cmp::min(length, MMAP_PRE_ALLOC_PAGES * PAGE_SIZE);
        for offset_aligned in (offset..offset + length).step_by(PAGE_SIZE) {
            if let Some(page) = block_on(async { file.get_page_at(offset_aligned).await })? {
                let vpn = range_vpn.next().unwrap();
                if flags.contains(MmapFlags::MAP_PRIVATE) {
                    let (pte_flags, ppn) = {
                        let mut new_flags: PTEFlags = perm.into();
                        new_flags |= PTEFlags::COW;
                        new_flags.remove(PTEFlags::W);
                        (new_flags, page.ppn())
                    };
                    page_table.map(vpn, ppn, pte_flags);
                    vma.pages.insert(vpn, page);
                    unsafe { sfence_vma_vaddr(vpn.to_vaddr().into()) };
                } else {
                    page_table.map(vpn, page.ppn(), perm.into());
                    vma.pages.insert(vpn, page);
                    unsafe { sfence_vma_vaddr(vpn.to_vaddr().into()) };
                }
            } else {
                break;
            }
        }
        self.push_vma_lazily(vma);
        Ok(start)
    }

    fn split_area(
        &self,
        old_range: Range<VirtAddr>,
        split_range: Range<VirtAddr>,
    ) -> (
        Option<&mut VmArea>,
        Option<&mut VmArea>,
        Option<&mut VmArea>,
    ) {
        let area = self.areas_mut().force_remove_one(old_range);
        let (left, middle, right) = area.split(split_range);
        let left_ret = left.map(|left| self.areas_mut().try_insert(left.range_va(), left).unwrap());
        let right_ret = right.map(|right| {
            self.areas_mut()
                .try_insert(right.range_va(), right)
                .unwrap()
        });
        let middle_ret = middle.map(|middle| {
            self.areas_mut()
                .try_insert(middle.range_va(), middle)
                .unwrap()
        });
        (left_ret, middle_ret, right_ret)
    }

    pub fn unmap(&mut self, range: Range<VirtAddr>) -> SysResult<()> {
        debug_assert!(range.start.is_aligned());
        debug_assert!(range.end.is_aligned());
        log::debug!("[MemorySpace::unmap] remove area {:?}", range.clone());

        // First find the left most vm_area containing `range.start`.
        if let Some((first_range, first_vma)) = self.areas_mut().get_key_value_mut(range.start) {
            if first_range.start >= range.start && first_range.end <= range.end {
                log::debug!(
                    "[MemorySpace::unmap] remove left most area {:?}",
                    first_range.clone()
                );
                let mut vma = self.areas_mut().force_remove_one(first_range);
                vma.unmap(self.page_table_mut());
            } else {
                // do split and unmap
                let split_range = range.start..cmp::min(range.end, first_range.end);
                log::debug!("[MemorySpace::unmap] split and remove left most vma {first_vma:?} in range {split_range:?}");
                let (_, middle, _) = self.split_area(first_range, split_range);
                if let Some(middle) = middle {
                    let mut vma = self.areas_mut().force_remove_one(middle.range_va());
                    vma.unmap(self.page_table_mut());
                }
            }
        }
        for (r, vma) in self.areas_mut().range_mut(range.clone()) {
            if r.start >= range.start && r.end <= range.end {
                log::debug!("[MemorySpace::unmap] remove area {:?}", r);
                let mut vma = self.areas_mut().force_remove_one(r);
                vma.unmap(self.page_table_mut());
            } else if r.end > range.end {
                // do split and unmap
                log::debug!(
                    "[MemorySpace::unmap] split and remove vma {vma:?} in range {:?}",
                    r.start..range.end
                );
                let (_, middle, _) = self.split_area(r.clone(), r.start..range.end);
                if let Some(middle) = middle {
                    let mut vma = self.areas_mut().force_remove_one(middle.range_va());
                    vma.unmap(self.page_table_mut());
                }
            }
        }
        Ok(())
    }

    pub fn mprotect(&mut self, range: Range<VirtAddr>, perm: MapPerm) -> SysResult<()> {
        debug_assert!(range.start.is_aligned() && range.end.is_aligned());
        let (old_range, area) = self
            .areas_mut()
            .get_key_value_mut(range.start)
            .ok_or(SysError::ENOMEM)?;
        if range == old_range {
            area.set_perm_and_flush(self.page_table_mut(), perm);
        } else {
            debug_assert!(old_range.end >= range.end);
            // do split and remap
            let (_, middle, _) = self.split_area(old_range, range);
            if let Some(middle) = middle {
                middle.set_perm_and_flush(self.page_table_mut(), perm);
            }
        }
        Ok(())
    }

    pub fn handle_page_fault(
        &mut self,
        va: VirtAddr,
        access_type: PageFaultAccessType,
    ) -> SysResult<()> {
        log::trace!("[MemorySpace::handle_page_fault] {va:?}");
        let vm_area = self.areas_mut().get_mut(va.round_down()).ok_or_else(|| {
            log::error!("[handle_page_fault] no area containing {va:?}");
            SysError::EFAULT
        })?;
        vm_area.handle_page_fault(self.page_table_mut(), va.floor(), access_type)?;
        Ok(())
    }

    pub unsafe fn switch_page_table(&self) {
        self.page_table().switch();
    }
}

pub fn init_stack(
    sp_init: VirtAddr,
    args: Vec<String>,
    envp: Vec<String>,
    auxv: Vec<AuxHeader>,
) -> (usize, usize, usize, usize) {
    // spec says:
    //      In the standard RISC-V calling convention, the stack grows downward
    //      and the stack pointer is always kept 16-byte aligned.

    // 参考：https://www.cnblogs.com/likaiming/p/11193697.html
    // 初始化之后的栈应该长这样子：
    // content                         size(bytes) + comment
    // -----------------------------------------------------------------------------
    //
    // [argc = number of args]         8
    // [argv[0](pointer)]              8
    // [argv[1](pointer)]              8
    // [argv[...](pointer)]            8 * x
    // [argv[n-1](pointer)]            8
    // [argv[n](pointer)]              8 (=NULL)
    //
    // [envp[0](pointer)]              8
    // [envp[1](pointer)]              8
    // [envp[..](pointer)]             8 * x
    // [envp[term](pointer)]           8 (=NULL)
    //
    // [auxv[0](Elf64_auxv_t)]         16
    // [auxv[1](Elf64_auxv_t)]         16
    // [auxv[..](Elf64_auxv_t)]        16 * x
    // [auxv[term](Elf64_auxv_t)]      16 (=NULL)
    //
    // [padding]                       >= 0
    // [rand bytes]                    16
    // [String identifying platform]   >= 0
    // [padding for align]             >= 0 (sp - (get_random_int() % 8192)) &
    // (~0xf)
    //
    // [argument ASCIIZ strings]       >= 0
    // [environment ASCIIZ str]        >= 0
    // --------------------------------------------------------------------------------
    // 在构建栈的时候，我们从底向上塞各个东西

    let mut sp = sp_init.bits();
    debug_assert!(sp & 0xf == 0);

    // 存放环境与参数的字符串本身
    fn push_str(sp: &mut usize, s: &str) -> usize {
        let len = s.len();
        *sp -= len + 1; // +1 for NUL ('\0')
        unsafe {
            // core::ptr::copy_nonoverlapping(s.as_ptr(), *sp as *mut u8, len);
            for (i, c) in s.bytes().enumerate() {
                log::trace!(
                    "push_str: {:x} ({:x}) <- {:?}",
                    *sp + i,
                    i,
                    core::str::from_utf8_unchecked(&[c])
                );
                *((*sp as *mut u8).add(i)) = c;
            }
            *(*sp as *mut u8).add(len) = 0u8;
        }
        *sp
    }

    let env_ptrs: Vec<usize> = envp.iter().rev().map(|s| push_str(&mut sp, s)).collect();
    let arg_ptrs: Vec<usize> = args.iter().rev().map(|s| push_str(&mut sp, s)).collect();

    // 随机对齐 (我们取 0 长度的随机对齐), 平台标识符，随机数与对齐
    fn align16(sp: &mut usize) {
        *sp = (*sp - 1) & !0xf;
    }

    let rand_size = 0;
    let platform = "RISC-V64";
    let rand_bytes = "Meow~ O4 here;D"; // 15 + 1 char for 16bytes

    sp -= rand_size;
    push_str(&mut sp, platform);
    push_str(&mut sp, rand_bytes);
    align16(&mut sp);

    // 存放 auxv
    fn push_aux_elm(sp: &mut usize, elm: &AuxHeader) {
        *sp -= core::mem::size_of::<AuxHeader>();
        unsafe {
            core::ptr::write(*sp as *mut AuxHeader, *elm);
        }
    }
    // 注意推栈是 "倒着" 推的，所以先放 null, 再逆着放别的
    push_aux_elm(&mut sp, &AuxHeader::new(AT_NULL, 0));
    for aux in auxv.into_iter().rev() {
        push_aux_elm(&mut sp, &aux);
    }

    // 存放 envp 与 argv 指针
    fn push_usize(sp: &mut usize, ptr: usize) {
        *sp -= core::mem::size_of::<usize>();
        log::debug!("addr: 0x{:x}, content: {:x}", *sp, ptr);
        unsafe {
            core::ptr::write(*sp as *mut usize, ptr);
        }
    }

    push_usize(&mut sp, 0);
    env_ptrs.iter().for_each(|ptr| push_usize(&mut sp, *ptr));
    let env_ptr_ptr = sp;

    push_usize(&mut sp, 0);
    arg_ptrs.iter().for_each(|ptr| push_usize(&mut sp, *ptr));
    let arg_ptr_ptr = sp;

    // 存放 argc
    let argc = args.len();
    push_usize(&mut sp, argc);

    // 返回值
    (sp, argc, arg_ptr_ptr, env_ptr_ptr)
}
