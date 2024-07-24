//! Memory management implementation
//!
//! SV39 page-based virtual-memory architecture for RV64 systems, and
//! everything about memory management, like frame allocator, page table,
//! map area and memory set, is implemented here.
//!
//! Every task or process has a memory_space to control its virtual memory.

pub mod memory_space;
mod user_ptr;

use core::cmp;

use config::{
    board::MEMORY_END,
    mm::{K_SEG_DTB_BEG, K_SEG_DTB_END, MAX_DTB_SIZE, PAGE_SIZE, VIRT_RAM_OFFSET},
};
pub use memory::page_table::PageTable;
use memory::{frame, heap, pte::PTEFlags, VirtAddr};
pub use memory_space::MemorySpace;
pub use user_ptr::{
    FutexAddr, PageFaultAccessType, UserMut, UserRdWrPtr, UserReadPtr, UserRef, UserSlice,
    UserWritePtr,
};

use self::memory_space::vm_area::MapPerm;
use crate::mm;

/// Initialize heap allocator, frame allocator and kernel page table.
pub fn init() {
    extern "C" {
        fn _ekernel();
    }
    heap::init_heap_allocator();
    frame::init_frame_allocator(
        VirtAddr::from(_ekernel as usize).to_offset().to_pa().into(),
        VirtAddr::from(MEMORY_END).to_offset().to_pa().into(),
    );
    unsafe {
        init_kernel_page_table();
        switch_kernel_page_table()
    };
    log::info!("KERNEL SPACE activated");
}

/// Kernel space for all processes.
///
/// There is no need to lock `KERNEL_PAGE_TABLE` since it won't be changed.
static mut KERNEL_PAGE_TABLE: Option<PageTable> = None;

unsafe fn init_kernel_page_table() {
    extern "C" {
        fn _stext();
        fn _strampoline();
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

    let mut kernel_page_table = PageTable::new();
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
        MEMORY_END
    );
    log::debug!("[kernel] mapping .text section");
    kernel_page_table.map_kernel_region(
        (_stext as usize).into()..(_strampoline as usize).into(),
        PTEFlags::R | PTEFlags::X,
    );
    log::debug!("[kernel] mapping signal-return trampoline");
    kernel_page_table.map_kernel_region(
        (_strampoline as usize).into()..(_etrampoline as usize).into(),
        PTEFlags::U | PTEFlags::R | PTEFlags::X,
    );
    kernel_page_table.map_kernel_region(
        (_etrampoline as usize).into()..(_etext as usize).into(),
        PTEFlags::R | PTEFlags::X,
    );
    log::debug!("[kernel] mapping .rodata section");
    kernel_page_table.map_kernel_region(
        (_srodata as usize).into()..(_erodata as usize).into(),
        PTEFlags::R,
    );
    log::debug!("[kernel] mapping .data section");
    kernel_page_table.map_kernel_region(
        (_sdata as usize).into()..(_edata as usize).into(),
        PTEFlags::R | PTEFlags::W,
    );
    log::debug!("[kernel] mapping .stack section");
    kernel_page_table.map_kernel_region(
        (_sstack as usize).into()..(_estack as usize).into(),
        PTEFlags::R | PTEFlags::W,
    );
    log::debug!("[kernel] mapping .bss section");
    kernel_page_table.map_kernel_region(
        (_sbss as usize).into()..(_ebss as usize).into(),
        PTEFlags::R | PTEFlags::W,
    );
    log::debug!("[kernel] mapping physical memory");
    kernel_page_table.map_kernel_region(
        (_ekernel as usize).into()..MEMORY_END.into(),
        PTEFlags::R | PTEFlags::W,
    );

    let dtb_addr = config::mm::dtb_addr();
    let dtb_end = cmp::min(MEMORY_END - VIRT_RAM_OFFSET, dtb_addr + MAX_DTB_SIZE);
    log::debug!("dtb address {dtb_addr:#x}, dtb end {dtb_end:#x}");
    kernel_page_table.map_kernel_region_offset(
        K_SEG_DTB_BEG.into()..(K_SEG_DTB_BEG + (dtb_end - dtb_addr)).into(),
        dtb_addr.into()..dtb_end.into(),
        PTEFlags::R | PTEFlags::W,
    );

    log::debug!("[kernel] KERNEL SPACE init finished");

    KERNEL_PAGE_TABLE = Some(kernel_page_table);
}

pub fn kernel_page_table() -> &'static PageTable {
    unsafe { KERNEL_PAGE_TABLE.as_ref().unwrap() }
}

/// # Safety
///
/// Should only hold mut ref when kernel init.
pub fn kernel_page_table_mut() -> &'static mut PageTable {
    unsafe { KERNEL_PAGE_TABLE.as_mut().unwrap() }
}

pub unsafe fn switch_kernel_page_table() {
    kernel_page_table().switch();
}
