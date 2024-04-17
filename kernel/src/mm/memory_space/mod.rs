use alloc::{collections::BTreeMap, vec::Vec};
use core::ops::Range;

use config::{
    board::MEMORY_END,
    mm::{
        PAGE_SIZE, USER_STACK_SIZE, U_SEG_HEAP_BEG, U_SEG_HEAP_END, U_SEG_STACK_BEG,
        U_SEG_STACK_END, VIRT_RAM_OFFSET,
    },
};
use log::info;
use memory::{pte::PTEFlags, PageTable, VPNRange, VirtAddr, VirtPageNum};
use spin::Lazy;
use sync::mutex::SpinNoIrqLock;
use systype::{SysError, SysResult};
use xmas_elf::ElfFile;

use self::{range_map::RangeMap, vm_area::VmArea};
use super::{page, user_ptr::PageFaultAccessType};
use crate::{
    mm::{
        memory_space::vm_area::{MapPerm, VmAreaType},
        MMIO,
    },
    processor::env::SumGuard,
    task::aux::{generate_early_auxv, AuxHeader, AT_BASE, AT_PHDR},
};

mod range_map;
pub mod vm_area;

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

/// Kernel space for all processes.
///
/// There is no need to lock `KERNEL_SPACE` since it won't be changed.
static KERNEL_SPACE: Lazy<MemorySpace> = Lazy::new(MemorySpace::new_kernel);

pub unsafe fn switch_kernel_page_table() {
    KERNEL_SPACE.switch_page_table();
}

pub struct MemorySpace {
    page_table: PageTable,
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
        log::info!("[kernel] trampoline {:#x}", sigreturn_trampoline as usize);
        log::info!(
            "[kernel] .text [{:#x}, {:#x}) [{:#x}, {:#x})",
            _stext as usize,
            _strampoline as usize,
            _etrampoline as usize,
            _etext as usize
        );
        log::info!(
            "[kernel] .text.trampoline [{:#x}, {:#x})",
            _strampoline as usize,
            _etrampoline as usize,
        );
        log::info!(
            "[kernel] .rodata [{:#x}, {:#x})",
            _srodata as usize,
            _erodata as usize
        );
        log::info!(
            "[kernel] .data [{:#x}, {:#x})",
            _sdata as usize,
            _edata as usize
        );
        log::info!(
            "[kernel] .stack [{:#x}, {:#x})",
            _sstack as usize,
            _estack as usize
        );
        log::info!(
            "[kernel] .bss [{:#x}, {:#x})",
            _sbss as usize,
            _ebss as usize
        );
        log::info!(
            "[kernel] physical mem [{:#x}, {:#x})",
            _ekernel as usize,
            MEMORY_END as usize
        );

        let mut memory_space = Self::new();
        log::info!("[kernel] mapping .text section");
        memory_space.push_vma(VmArea::new(
            (_stext as usize).into()..(_strampoline as usize).into(),
            MapPerm::RX,
            VmAreaType::Physical,
        ));
        memory_space.push_vma(VmArea::new(
            (_etrampoline as usize).into()..(_etext as usize).into(),
            MapPerm::RX,
            VmAreaType::Physical,
        ));
        log::info!("[kernel] mapping .rodata section");
        memory_space.push_vma(VmArea::new(
            (_srodata as usize).into()..(_erodata as usize).into(),
            MapPerm::R,
            VmAreaType::Physical,
        ));
        log::info!("[kernel] mapping .data section");
        memory_space.push_vma(VmArea::new(
            (_sdata as usize).into()..(_edata as usize).into(),
            MapPerm::RW,
            VmAreaType::Physical,
        ));
        log::info!("[kernel] mapping .stack section");
        memory_space.push_vma(VmArea::new(
            (_sstack as usize).into()..(_estack as usize).into(),
            MapPerm::RW,
            VmAreaType::Physical,
        ));
        log::info!("[kernel] mapping .bss section");
        memory_space.push_vma(VmArea::new(
            (_sbss as usize).into()..(_ebss as usize).into(),
            MapPerm::RW,
            VmAreaType::Physical,
        ));
        log::info!("[kernel] mapping signal-return trampoline");
        memory_space.push_vma(VmArea::new(
            (_strampoline as usize).into()..(_etrampoline as usize).into(),
            MapPerm::URX,
            VmAreaType::Physical,
        ));
        log::info!("[kernel] mapping physical memory");
        memory_space.push_vma(VmArea::new(
            (_ekernel as usize).into()..MEMORY_END.into(),
            MapPerm::RW,
            VmAreaType::Physical,
        ));
        log::info!("[kernel] mapping mmio registers");
        for pair in MMIO {
            memory_space.push_vma(VmArea::new(
                (pair.0 + VIRT_RAM_OFFSET).into()..(pair.0 + pair.1 + VIRT_RAM_OFFSET).into(),
                pair.2,
                VmAreaType::Mmio,
            ));
        }
        log::info!("[kernel] KERNEL SPACE init finished");
        memory_space
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
            let mut vm_area = VmArea::new(start_va..end_va, map_perm, VmAreaType::Elf);

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
        let mut entry_point = elf_header.pt2.entry_point() as usize;
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
        let mut user_stack_bottom: usize = usize::from(max_end_va) + PAGE_SIZE;
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;
        let mut ustack_vma = VmArea::new(
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

        let (max_end_vpn, header_va) = self.map_elf(&elf, 0.into());

        let ph_head_addr = header_va.0 + elf.header.pt2.ph_offset() as usize;
        log::debug!("[parse_and_map_elf] AT_PHDR  ph_head_addr is {ph_head_addr:x}",);
        auxv.push(AuxHeader::new(AT_PHDR, ph_head_addr));

        (entry, auxv)
    }

    /// Alloc stack and map it in the page table.
    ///
    /// Return address of the stack top, which is aligned to 16 bytes.
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
        log::debug!("alloc stack: {range:x?}, sp_init: {sp_init:x?}");

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
        let (range, vma) = self
            .areas
            .iter_mut()
            .find(|(_, vma)| vma.vma_type == VmAreaType::Heap)
            .unwrap();
        let result = if new_brk > range.end {
            self.areas.extend_back(range.start..new_brk)
        } else if new_brk < range.end {
            self.areas.reduce_back(range.start, new_brk)
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
        for (_, area) in user_space.areas.iter() {
            let mut new_area = VmArea::from_another(area);
            memory_space.push_vma(new_area);
            // copy data from another space
            for vpn in area.vpn_range.clone() {
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
        for (_, area) in user_space.areas.iter() {
            log::debug!("[MemorySpace::from_user_lazily] cloning area {area:?}");
            let new_area = area.clone();
            for vpn in area.vpn_range.clone() {
                if let Some(page) = area.pages.get(&vpn) {
                    let pte = user_space.page_table.find_pte(vpn).unwrap();
                    let (pte_flags, ppn) = match area.vma_type {
                        VmAreaType::Shm => {
                            // If shared memory,
                            // then we don't need to modify the pte flags,
                            // i.e. no copy-on-write.
                            todo!()
                        }
                        _ => {
                            // Else,
                            // copy-on-write
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
    pub fn push_vma_lazily(&mut self, mut vma: VmArea) {
        self.areas.try_insert(vma.range_va(), vma).unwrap();
    }

    /// Push `VmArea` into `MemorySpace` and map it in page table, also copy
    /// `data` at `offset` of `vma`.
    pub fn push_vma_with_data(&mut self, mut vma: VmArea, offset: usize, data: &[u8]) {
        vma.map(&mut self.page_table);
        vma.copy_data_with_offset(&self.page_table, offset, data);
        self.areas.try_insert(vma.range_va(), vma).unwrap();
    }

    pub fn handle_page_fault(
        &mut self,
        va: VirtAddr,
        access_type: PageFaultAccessType,
    ) -> SysResult<()> {
        log::trace!("[MemorySpace::handle_page_fault] {va:?}");
        let vm_area = self.areas.get_mut(va).expect("no area contains this va");
        vm_area.handle_page_fault(&mut self.page_table, va.floor());
        Ok(())
    }

    pub unsafe fn switch_page_table(&self) {
        self.page_table.switch();
    }

    /// only for debug
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

            for vpn in area.vpn_range.clone() {
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
