use alloc::{
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::ops::Range;

use async_utils::block_on;
use config::{
    board::MEMORY_END,
    mm::{
        PAGE_SIZE, USER_STACK_SIZE, U_SEG_FILE_BEG, U_SEG_FILE_END, U_SEG_HEAP_BEG, U_SEG_HEAP_END,
        U_SEG_SHARE_BEG, U_SEG_SHARE_END, U_SEG_STACK_BEG, U_SEG_STACK_END, VIRT_RAM_OFFSET,
    },
};
use log::info;
use memory::{page::Page, pte::PTEFlags, PageTable, VirtAddr, VirtPageNum};
use spin::Lazy;
use systype::{SysError, SysResult};
use vfs_core::File;
use xmas_elf::ElfFile;

use self::{range_map::RangeMap, vm_area::VmArea};
use super::PageFaultAccessType;
use crate::{
    mm::{
        memory_space::vm_area::{MapPerm, VmAreaType},
        user_ptr::UserSlice,
        MMIO,
    },
    processor::env::SumGuard,
    syscall::MmapFlags,
    task::aux::{generate_early_auxv, AuxHeader, AT_BASE, AT_NULL, AT_PHDR},
};

mod range_map;
pub mod vm_area;

/// Kernel space for all processes.
///
/// There is no need to lock `KERNEL_SPACE` since it won't be changed.
static KERNEL_SPACE: Lazy<MemorySpace> = Lazy::new(MemorySpace::new_kernel);

pub unsafe fn switch_kernel_page_table() {
    KERNEL_SPACE.switch_page_table();
}

/// Virtual memory space for kernel and user.
pub struct MemorySpace {
    /// Page table of this memory space.
    page_table: PageTable,
    /// Map of `VmArea`s in this memory space.
    /// NOTE: stores range that is lazy allocated
    areas: RangeMap<VirtAddr, VmArea>,
}

impl MemorySpace {
    /// Create an empty `MemorySpace`
    pub fn new() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: RangeMap::new(),
        }
    }

    /// Create a new user memory space that inherits kernel page table.
    pub fn new_user() -> Self {
        Self {
            page_table: PageTable::from_kernel(&KERNEL_SPACE.page_table),
            areas: RangeMap::new(),
        }
    }

    /// Create a kernel space.
    pub fn new_kernel() -> Self {
        extern "C" {
            fn _stext();
            fn _strampoline();
            fn sigreturn_trampoline();
            fn _etrampoline();
            fn _etext();
            fn _srodata();
            fn _erodata();
            fn _sdata();
            fn _edata();
            fn _sstack();
            fn _estack();
            fn _sbss();
            fn _ebss();
            fn _ekernel();
        }

        log::debug!("[kernel] trampoline {:#x}", sigreturn_trampoline as usize);
        log::debug!(
            "[kernel] .text [{:#x}, {:#x}) [{:#x}, {:#x})",
            _stext as usize,
            _strampoline as usize,
            _etrampoline as usize,
            _etext as usize
        );
        log::debug!(
            "[kernel] .text.trampoline [{:#x}, {:#x})",
            _strampoline as usize,
            _etrampoline as usize,
        );
        log::debug!(
            "[kernel] .rodata [{:#x}, {:#x})",
            _srodata as usize,
            _erodata as usize
        );
        log::debug!(
            "[kernel] .data [{:#x}, {:#x})",
            _sdata as usize,
            _edata as usize
        );
        log::debug!(
            "[kernel] .stack [{:#x}, {:#x})",
            _sstack as usize,
            _estack as usize
        );
        log::debug!(
            "[kernel] .bss [{:#x}, {:#x})",
            _sbss as usize,
            _ebss as usize
        );
        log::debug!(
            "[kernel] physical mem [{:#x}, {:#x})",
            _ekernel as usize,
            MEMORY_END
        );

        let mut memory_space = Self::new();
        log::debug!("[kernel] mapping .text section");
        memory_space.push_vma(VmArea::new(
            (_stext as usize).into()..(_strampoline as usize).into(),
            MapPerm::RX,
            VmAreaType::Physical,
        ));
        log::debug!("[kernel] mapping signal-return trampoline");
        memory_space.push_vma(VmArea::new(
            (_strampoline as usize).into()..(_etrampoline as usize).into(),
            MapPerm::URX,
            VmAreaType::Physical,
        ));
        memory_space.push_vma(VmArea::new(
            (_etrampoline as usize).into()..(_etext as usize).into(),
            MapPerm::RX,
            VmAreaType::Physical,
        ));
        log::debug!("[kernel] mapping .rodata section");
        memory_space.push_vma(VmArea::new(
            (_srodata as usize).into()..(_erodata as usize).into(),
            MapPerm::R,
            VmAreaType::Physical,
        ));
        log::debug!("[kernel] mapping .data section");
        memory_space.push_vma(VmArea::new(
            (_sdata as usize).into()..(_edata as usize).into(),
            MapPerm::RW,
            VmAreaType::Physical,
        ));
        log::debug!("[kernel] mapping .stack section");
        memory_space.push_vma(VmArea::new(
            (_sstack as usize).into()..(_estack as usize).into(),
            MapPerm::RW,
            VmAreaType::Physical,
        ));
        log::debug!("[kernel] mapping .bss section");
        memory_space.push_vma(VmArea::new(
            (_sbss as usize).into()..(_ebss as usize).into(),
            MapPerm::RW,
            VmAreaType::Physical,
        ));
        log::debug!("[kernel] mapping physical memory");
        memory_space.push_vma(VmArea::new(
            (_ekernel as usize).into()..MEMORY_END.into(),
            MapPerm::RW,
            VmAreaType::Physical,
        ));
        log::debug!("[kernel] mapping mmio registers");
        for pair in MMIO {
            memory_space.push_vma(VmArea::new(
                (pair.0 + VIRT_RAM_OFFSET).into()..(pair.0 + pair.1 + VIRT_RAM_OFFSET).into(),
                pair.2,
                VmAreaType::Mmio,
            ));
        }
        log::debug!("[kernel] KERNEL SPACE init finished");
        memory_space
    }

    pub fn areas_mut(&mut self) -> &mut RangeMap<VirtAddr, VmArea> {
        &mut self.areas
    }

    pub fn page_table_mut(&mut self) -> &mut PageTable {
        &mut self.page_table
    }

    /// Map the sections in the elf.
    ///
    /// Return the max end vpn and the first section's va.
    pub fn map_elf(&mut self, elf: &ElfFile, offset: VirtAddr) -> (VirtPageNum, VirtAddr) {
        let elf_header = elf.header;
        let ph_count = elf_header.pt2.ph_count();

        let mut max_end_vpn = offset.floor();
        let mut header_va = 0;
        let mut has_found_header_va = false;
        info!("[map_elf]: entry point {:#x}", elf.header.pt2.entry_point());

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
            let vm_area = VmArea::new(start_va..end_va, map_perm, VmAreaType::Elf);

            log::debug!("[map_elf] [{start_va:#x}, {end_va:#x}], map_perm: {map_perm:?} start...",);

            max_end_vpn = vm_area.end_vpn();

            let map_offset = start_va.0 - start_va.floor().0 * PAGE_SIZE;

            log::debug!(
                "[map_elf] ph offset {:#x}, file size {:#x}, mem size {:#x}",
                ph.offset(),
                ph.file_size(),
                ph.mem_size()
            );

            self.push_vma_with_data(
                vm_area,
                map_offset,
                &elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize],
            );

            log::info!("[map_elf] [{start_va:#x}, {end_va:#x}], map_perm: {map_perm:?}",);
        }

        (max_end_vpn, header_va.into())
    }

    /// Include sections in elf and TrapContext and user stack,
    /// also returns user_sp and entry point.
    // PERF: resolve elf file lazily
    // TODO: dynamic interpreter
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize, Vec<AuxHeader>) {
        const ELF_MAGIC: [u8; 4] = [0x7f, 0x45, 0x4c, 0x46];

        let mut memory_space = Self::new_user();

        // map program headers of elf, with U flag
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        assert_eq!(elf_header.pt1.magic, ELF_MAGIC, "invalid elf!");
        let entry_point = elf_header.pt2.entry_point() as usize;
        let ph_entry_size = elf_header.pt2.ph_entry_size() as usize;
        let ph_count = elf_header.pt2.ph_count() as usize;

        let mut auxv = generate_early_auxv(ph_entry_size, ph_count, entry_point);

        auxv.push(AuxHeader::new(AT_BASE, 0));

        let (max_end_vpn, header_va) = memory_space.map_elf(&elf, 0.into());

        let ph_head_addr = header_va.0 + elf.header.pt2.ph_offset() as usize;
        log::debug!("[from_elf] AT_PHDR  ph_head_addr is {ph_head_addr:x} ");
        auxv.push(AuxHeader::new(AT_PHDR, ph_head_addr));

        // map user stack with U flags
        let max_end_va: VirtAddr = max_end_vpn.into();
        let user_stack_bottom: usize = usize::from(max_end_va) + PAGE_SIZE;
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;
        let ustack_vma = VmArea::new(
            user_stack_bottom.into()..user_stack_top.into(),
            MapPerm::URW,
            VmAreaType::Stack,
        );
        memory_space.push_vma(ustack_vma);
        log::info!("[from_elf] map ustack: {user_stack_bottom:#x}, {user_stack_top:#x}",);

        memory_space.alloc_heap_lazily();

        // // guard page
        // let heap_start_va = user_stack_top + PAGE_SIZE;
        // let heap_end_va = heap_start_va;
        // let mut heap_vma = VmArea::new(
        //     heap_start_va.into(),
        //     heap_end_va.into(),
        //     MapPerm::URW,
        //     VmAreaType::Heap,
        // );
        // memory_space.push_vma(heap_vma);
        // log::info!(
        //     "[from_elf] map heap: {:#x}, {:#x}",
        //     heap_start_va,
        //     heap_end_va
        // );
        (memory_space, user_stack_top, entry_point, auxv)
    }

    pub fn parse_and_map_elf(&mut self, elf_data: &[u8]) -> (usize, Vec<AuxHeader>) {
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

        let (_max_end_vpn, header_va) = self.map_elf(&elf, 0.into());

        let ph_head_addr = header_va.0 + elf.header.pt2.ph_offset() as usize;
        log::debug!("[parse_and_map_elf] AT_PHDR  ph_head_addr is {ph_head_addr:x}",);
        auxv.push(AuxHeader::new(AT_PHDR, ph_head_addr));

        (entry, auxv)
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
                .areas
                .find_free_range(SHARED_RANGE, size)
                .expect("no free shared area");
            ret_addr = range.start;
            VmArea::new(range, map_perm, VmAreaType::Shm)
        } else {
            log::warn!("[attach_shm] user defined addr");
            let shm_end = shmaddr + size;
            VmArea::new(shmaddr..shm_end, map_perm, VmAreaType::Shm)
        };
        if pages.is_empty() {
            for vpn in vm_area.range_vpn() {
                let page = Arc::new(Page::new());
                self.page_table.map(vpn, page.ppn(), map_perm.into());
                pages.push(Arc::downgrade(&page));
                vm_area.pages.insert(vpn, page);
            }
        } else {
            debug_assert!(pages.len() == vm_area.range_vpn().end - vm_area.range_vpn().start);
            let mut pages = pages.iter();
            for vpn in vm_area.range_vpn() {
                let page = pages.next().unwrap().upgrade().unwrap();
                self.page_table.map(vpn, page.ppn(), map_perm.into());
                vm_area.pages.insert(vpn, page.clone());
            }
        }
        self.push_vma(vm_area);
        return ret_addr;
    }

    /// `shmaddr` must be the return value of shmget (i.e. `shmaddr` is page
    /// aligned and in the beginning of the vm_area with type Shm). The
    /// check should be done at the caller who call `detach_shm`
    pub fn detach_shm(&mut self, shmaddr: VirtAddr) {
        let mut range_to_remove = None;
        if let Some((range, vm_area)) = self.areas.iter().find(|(range, _)| range.start == shmaddr)
        {
            if vm_area.vma_type != VmAreaType::Shm {
                panic!("[detach_shm] 'vm_area.vma_type != VmAreaType::Shm' this won't happen");
            }
            log::warn!("[detach_shm] try to remove {:?}", range);
            range_to_remove = Some(range);
            for vpn in vm_area.range_vpn() {
                self.page_table.unmap(vpn);
            }
        } else {
            panic!("[detach_shm] this won't happen");
        }
        if let Some(range) = range_to_remove {
            self.areas.force_remove_one(range);
        } else {
            panic!("[detach_shm] range_to_remove is None! This should never happen");
        }
    }

    /// Alloc stack and map it in the page table.
    ///
    /// Return the address of the stack top, which is aligned to 16 bytes.
    ///
    /// The stack has a range of [sp - size, sp].
    pub fn alloc_stack(&mut self, size: usize) -> VirtAddr {
        const STACK_RANGE: Range<VirtAddr> =
            VirtAddr::from_usize_range(U_SEG_STACK_BEG..U_SEG_STACK_END);

        let range = self
            .areas
            .find_free_range(STACK_RANGE, size)
            .expect("too many stack!");

        // align to 16 bytes
        let sp_init = VirtAddr::from((range.end.bits() - 1) & !0xf);
        log::debug!("[MemorySpace::alloc_stack] stack: {range:x?}, sp_init: {sp_init:x?}");

        let vm_area = VmArea::new(range, MapPerm::URW, VmAreaType::Stack);
        self.push_vma(vm_area);
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
            .areas
            .iter()
            .find(|(_, vma)| vma.vma_type == VmAreaType::Heap)
            .unwrap();
        range.end
    }

    /// NOTE: The actual Linux system call returns the new program break on
    /// success. On failure, the system call returns the current break.
    pub fn reset_heap_break(&mut self, new_brk: VirtAddr) -> VirtAddr {
        let (range, _vma) = self
            .areas
            .iter_mut()
            .find(|(_, vma)| vma.vma_type == VmAreaType::Heap)
            .unwrap();
        log::debug!("[MemorySpace::reset_heap_break] heap range: {range:?}, new_brk: {new_brk:?}");
        let result = if new_brk > range.end {
            let ret = self.areas.extend_back(range.start..new_brk);
            if ret.is_ok() {
                let (range_va, vm_area) = self.areas.get_key_value_mut(range.start).unwrap();
                vm_area.set_range_va(range_va);
            }
            ret
        } else if new_brk < range.end {
            let ret = self.areas.reduce_back(range.start, new_brk);
            if ret.is_ok() {
                let (range_va, vm_area) = self.areas.get_key_value_mut(range.start).unwrap();
                vm_area.set_range_va(range_va);
                let range_vpn: Range<VirtPageNum> = new_brk.ceil()..range.end.ceil();
                vm_area.unmap(&mut self.page_table, range_vpn);
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

    /// Clone a same `MemorySpace` from another user space, including datas in
    /// memory.
    pub fn from_user(user_space: &Self) -> Self {
        let mut memory_space = Self::new_user();
        for (range, area) in user_space.areas.iter() {
            let new_area = VmArea::from_another(area);
            debug_assert_eq!(range, new_area.range_va());
            memory_space.push_vma(new_area);
            // copy data from another space
            for vpn in area.range_vpn() {
                if let Some(pte) = user_space.page_table.find_pte(vpn) {
                    let src_ppn = pte.ppn();
                    let dst_ppn = memory_space.page_table.find_pte(vpn).unwrap().ppn();
                    dst_ppn.bytes_array().copy_from_slice(src_ppn.bytes_array());
                }
            }
        }
        memory_space
    }

    /// Clone a same `MemorySpace` lazily.
    pub fn from_user_lazily(user_space: &mut Self) -> Self {
        let mut memory_space = Self::new_user();
        for (range, area) in user_space.areas.iter() {
            log::debug!("[MemorySpace::from_user_lazily] cloning {area:?}");
            let mut new_area = area.clone();
            debug_assert_eq!(range, new_area.range_va());
            for vpn in area.range_vpn() {
                if let Some(page) = area.pages.get(&vpn) {
                    let pte = user_space.page_table.find_pte(vpn).unwrap();
                    let (pte_flags, ppn) = match area.vma_type {
                        VmAreaType::Shm => {
                            // If shared memory,
                            // then we don't need to modify the pte flags,
                            // i.e. no copy-on-write.
                            info!("[from_user_lazily] clone Shared Memory");
                            new_area.pages.insert(vpn, page.clone());
                            (pte.flags(), page.ppn())
                        }
                        _ => {
                            // copy on write
                            let mut new_flags = pte.flags() | PTEFlags::COW;
                            new_flags.remove(PTEFlags::W);
                            pte.set_flags(new_flags);
                            (new_flags, page.ppn())
                        }
                    };
                    memory_space.page_table.map(vpn, ppn, pte_flags);
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
        vma.map(&mut self.page_table);
        self.areas.try_insert(vma.range_va(), vma).unwrap();
    }

    /// Push `VmArea` into `MemorySpace` without mapping it in page table.
    pub fn push_vma_lazily(&mut self, vma: VmArea) {
        self.areas.try_insert(vma.range_va(), vma).unwrap();
    }

    /// Push `VmArea` into `MemorySpace` and map it in page table, also copy
    /// `data` at `offset` of `vma`.
    pub fn push_vma_with_data(&mut self, mut vma: VmArea, offset: usize, data: &[u8]) {
        vma.map(&mut self.page_table);
        vma.copy_data_with_offset(&self.page_table, offset, data);
        self.areas.try_insert(vma.range_va(), vma).unwrap();
    }

    pub fn alloc_mmap_private_anon(&mut self, perm: MapPerm, length: usize) -> SysResult<VirtAddr> {
        const MMAP_RANGE: Range<VirtAddr> =
            VirtAddr::from_usize_range(U_SEG_FILE_BEG..U_SEG_FILE_END);
        let range = self
            .areas
            .find_free_range(MMAP_RANGE, length)
            .expect("mmap range is full");
        let start = range.start;
        let mut vma = VmArea::new(range, perm, VmAreaType::Mmap);
        vma.map(&mut self.page_table);
        let mut buf = Vec::with_capacity(length);
        buf.fill(0);
        vma.copy_data_with_offset(&self.page_table, 0, &buf);
        self.areas.try_insert(vma.range_va(), vma).unwrap();
        Ok(start)
    }

    pub fn alloc_mmap_area(
        &mut self,
        length: usize,
        perm: MapPerm,
        flags: MmapFlags,
        file: Arc<dyn File>,
        offset: usize,
    ) -> SysResult<VirtAddr> {
        const MMAP_RANGE: Range<VirtAddr> =
            VirtAddr::from_usize_range(U_SEG_FILE_BEG..U_SEG_FILE_END);
        let range = self
            .areas
            .find_free_range(MMAP_RANGE, length)
            .expect("mmap range is full");
        let start = range.start;
        let mut vma = VmArea::new_mmap(range, perm, flags, Some(file.clone()), offset);
        vma.map(&mut self.page_table);
        let mut buf = unsafe { UserSlice::new_unchecked(vma.start_va(), length) };
        block_on(async { file.read_at(offset, &mut buf).await })?;
        vma.copy_data_with_offset(&self.page_table, 0, &buf);
        self.areas.try_insert(vma.range_va(), vma).unwrap();
        Ok(start)
    }

    pub fn handle_page_fault(
        &mut self,
        va: VirtAddr,
        access_type: PageFaultAccessType,
    ) -> SysResult<()> {
        log::trace!("[MemorySpace::handle_page_fault] {va:?}");
        let vm_area = self.areas.get_mut(va).ok_or_else(|| {
            log::error!("[handle_page_fault] no area containing {va:?}");
            SysError::EFAULT
        })?;
        vm_area.handle_page_fault(&mut self.page_table, va.floor(), access_type)?;
        Ok(())
    }

    pub unsafe fn switch_page_table(&self) {
        self.page_table.switch();
    }

    /// only for debug
    #[allow(unused)]
    pub fn print_all(&self) {
        use crate::{
            trap::{
                kernel_trap::{set_kernel_user_rw_trap, will_read_fail},
                set_kernel_trap,
            },
            utils::exam_hash,
        };
        let _sum_guard = SumGuard::new();
        unsafe { set_kernel_user_rw_trap() };
        for (range, area) in self.areas.iter() {
            log::warn!(
                "==== {:?}, {:?}, {:?}, ====",
                area.vma_type,
                area.perm(),
                range,
            );

            for vpn in area.range_vpn() {
                let vaddr = vpn.to_va();
                if will_read_fail(vaddr.bits()) {
                    // log::debug!("{:<8x}: unmapped", vpn);
                } else {
                    let hash = exam_hash(vpn.bytes_array());
                    log::info!(
                        "0x{: >8x}: {:0>4x} {:0>4x} {:0>4x} {:0>4x}",
                        vpn.0,
                        (hash & 0xffff_0000_0000_0000) >> 48,
                        (hash & 0x0000_ffff_0000_0000) >> 32,
                        (hash & 0x0000_0000_ffff_0000) >> 16,
                        (hash & 0x0000_0000_0000_ffff),
                    );
                }
            }
        }
        log::warn!("==== print all done ====");
        unsafe { set_kernel_trap() };
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
