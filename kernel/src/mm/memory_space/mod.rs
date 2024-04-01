use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use core::cell::SyncUnsafeCell;

use config::{board::MEMORY_END, mm::VIRT_RAM_OFFSET};
use log::info;
use memory::{
    address::SimpleRange, page_table, MapPermission, PageTable, VirtAddr, VirtPageNum, MMIO,
};
use spin::Lazy;
use sync::mutex::{SpinNoIrq, SpinNoIrqLock};

use self::vm_area::VmArea;
use crate::{
    mm::memory_space::{self, vm_area::VmAreaType},
    stack_trace,
    task::aux::AuxHeader,
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

/// Kernel Space for all processes
pub static mut KERNEL_SPACE: Lazy<SpinNoIrqLock<MemorySpace>> =
    Lazy::new(|| SpinNoIrqLock::new(MemorySpace::new_kernel()));

pub fn activate_kernel_space() {
    unsafe {
        KERNEL_SPACE.lock().activate();
    }
}

pub struct MemorySpace {
    page_table: PageTable,
    areas: Vec<VmArea>,
}

fn kernel_info() {
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
        stack_trace!();
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
        }
    }

    /// Create a kernel space
    pub fn new_kernel() -> Self {
        stack_trace!();
        kernel_info();
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
            MapPermission::R | MapPermission::W,
            VmAreaType::Physical,
        ));
        info!("[kernel] mapping signal-return trampoline");
        memory_space.areas.push(VmArea::new(
            (_strampoline as usize).into(),
            (_etrampoline as usize).into(),
            MapPermission::R | MapPermission::X | MapPermission::U,
            VmAreaType::Physical,
        ));
        info!("[kernel] mapping physical memory");
        memory_space.areas.push(VmArea::new(
            (_ekernel as usize).into(),
            MEMORY_END.into(),
            MapPermission::R | MapPermission::W,
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
        memory_space.activate();
        memory_space
    }

    /// Map the sections in the elf.
    /// Return the max end vpn and the first section's va.
    fn map_elf(&mut self, elf_data: &[u8]) -> (VirtPageNum, VirtAddr) {
        stack_trace!();
        todo!()
    }

    /// Include sections in elf and TrapContext and user stack,
    /// also returns user_sp and entry point.
    /// TODO: resolve elf file lazily
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize, Vec<AuxHeader>) {
        todo!()
    }

    pub fn activate(&self) {
        self.page_table.activate();
    }
}
