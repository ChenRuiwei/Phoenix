use alloc::vec::Vec;

use config::{
    board::MEMORY_END,
    mm::{PAGE_SIZE, USER_STACK_SIZE, VIRT_RAM_OFFSET},
};
use log::info;
use memory::{PageTable, VirtAddr, VirtPageNum};
use spin::Lazy;
use sync::mutex::SpinNoIrqLock;
use systype::SysResult;
use xmas_elf::ElfFile;

use self::vm_area::VmArea;
use super::user_ptr::PageFaultAccessType;
use crate::{
    mm::{
        memory_space::vm_area::{MapPermission, VmAreaType},
        MMIO,
    },
    task::aux::{generate_early_auxv, AuxHeader, AT_BASE, AT_PHDR},
};

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

/// Kernel Space for all processes.
///
/// There is no need to lock `KERNEL_SPACE` since it won't be changed.
/// initialization.
pub static KERNEL_SPACE: Lazy<MemorySpace> = Lazy::new(|| MemorySpace::new_kernel());

pub fn activate_kernel_space() {
    unsafe {
        KERNEL_SPACE.activate();
    }
}

pub struct MemorySpace {
    page_table: PageTable,
    areas: Vec<VmArea>,
}

fn kernel_space_info() {
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
}

impl MemorySpace {
    /// Create an empty `MemorySpace`
    pub fn new() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
        }
    }

    pub fn new_from_global() -> Self {
        Self {
            page_table: PageTable::from_global(&KERNEL_SPACE.page_table),
            areas: Vec::new(),
        }
    }

    /// Create a kernel space
    pub fn new_kernel() -> Self {
        kernel_space_info();
        let mut memory_space = Self::new();
        info!("[kernel] mapping .text section");
        memory_space.areas.push(VmArea::new(
            (_stext as usize).into(),
            (_strampoline as usize).into(),
            MapPermission::RX,
            VmAreaType::Physical,
        ));
        memory_space.areas.push(VmArea::new(
            (_etrampoline as usize).into(),
            (_etext as usize).into(),
            MapPermission::RX,
            VmAreaType::Physical,
        ));
        info!("[kernel] mapping .rodata section");
        memory_space.areas.push(VmArea::new(
            (_srodata as usize).into(),
            (_erodata as usize).into(),
            MapPermission::R,
            VmAreaType::Physical,
        ));
        info!("[kernel] mapping .data section");
        memory_space.areas.push(VmArea::new(
            (_sdata as usize).into(),
            (_edata as usize).into(),
            MapPermission::RW,
            VmAreaType::Physical,
        ));
        info!("[kernel] mapping .stack section");
        memory_space.areas.push(VmArea::new(
            (_sstack as usize).into(),
            (_estack as usize).into(),
            MapPermission::RW,
            VmAreaType::Physical,
        ));
        info!("[kernel] mapping .bss section");
        memory_space.areas.push(VmArea::new(
            (_sbss as usize).into(),
            (_ebss as usize).into(),
            MapPermission::RW,
            VmAreaType::Physical,
        ));
        info!("[kernel] mapping signal-return trampoline");
        memory_space.areas.push(VmArea::new(
            (_strampoline as usize).into(),
            (_etrampoline as usize).into(),
            MapPermission::URX,
            VmAreaType::Physical,
        ));
        info!("[kernel] mapping physical memory");
        memory_space.areas.push(VmArea::new(
            (_ekernel as usize).into(),
            MEMORY_END.into(),
            MapPermission::RW,
            VmAreaType::Physical,
        ));
        info!("[kernel] mapping mmio registers");
        for pair in MMIO {
            info!("start va: {:#x}", pair.0);
            info!("end va: {:#x}", pair.0 + pair.1);
            info!("permission: {:?}", pair.2);
            memory_space.areas.push(VmArea::new(
                (pair.0 + VIRT_RAM_OFFSET).into(),
                (pair.0 + pair.1 + VIRT_RAM_OFFSET).into(),
                pair.2,
                VmAreaType::Mmio,
            ));
        }
        info!("[kernel] new kernel finished");
        for area in memory_space.areas.iter_mut() {
            area.map(&mut memory_space.page_table);
        }
        memory_space
    }

    /// Map the sections in the elf.
    /// Return the max end vpn and the first section's va.
    fn map_elf(&mut self, elf: &ElfFile, offset: VirtAddr) -> (VirtPageNum, VirtAddr) {
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
            let mut vm_area = VmArea::new(start_va, end_va, map_perm, VmAreaType::Elf);

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

            log::debug!("{map_offset}");

            vm_area.map(&mut self.page_table);
            vm_area.copy_data_with_offset(
                &self.page_table,
                map_offset,
                &elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize],
            );
            self.areas.push(vm_area);

            log::info!(
                "[map_elf] [{:#x}, {:#x}], map_perm: {:?}",
                start_va.0,
                end_va.0,
                map_perm
            );
        }

        (max_end_vpn, header_va.into())
    }

    /// Include sections in elf and TrapContext and user stack,
    /// also returns user_sp and entry point.
    /// PERF: resolve elf file lazily
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize, Vec<AuxHeader>) {
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

        auxv.push(AuxHeader::new(AT_BASE, 0));

        let (max_end_vpn, header_va) = memory_space.map_elf(&elf, 0.into());

        let ph_head_addr = header_va.0 + elf.header.pt2.ph_offset() as usize;
        log::debug!("[from_elf] AT_PHDR  ph_head_addr is {:x} ", ph_head_addr);
        auxv.push(AuxHeader::new(AT_PHDR, ph_head_addr));

        // map user stack with U flags
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_bottom: usize = usize::from(max_end_va) + PAGE_SIZE;
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;
        let mut ustack_vma = VmArea::new(
            user_stack_bottom.into(),
            user_stack_top.into(),
            MapPermission::URW,
            VmAreaType::Stack,
        );
        ustack_vma.map(&mut memory_space.page_table);
        memory_space.areas.push(ustack_vma);

        log::info!(
            "[from_elf] map ustack: {:#x}, {:#x}",
            user_stack_bottom,
            user_stack_top,
        );

        // guard page
        let heap_start_va = user_stack_top + PAGE_SIZE;
        let heap_end_va = heap_start_va;
        let mut heap_vma = VmArea::new(
            heap_start_va.into(),
            heap_end_va.into(),
            MapPermission::URW,
            VmAreaType::Heap,
        );
        heap_vma.map(&mut memory_space.page_table);
        memory_space.areas.push(heap_vma);
        log::info!(
            "[from_elf] map heap: {:#x}, {:#x}",
            heap_start_va,
            heap_end_va
        );
        (memory_space, user_stack_top, entry_point, auxv)
    }

    pub fn handle_pagefault(
        &mut self,
        vaddr: VirtAddr,
        access_type: PageFaultAccessType,
    ) -> SysResult<()> {
        todo!();
    }

    pub fn activate(&self) {
        self.page_table.activate();
    }
}
