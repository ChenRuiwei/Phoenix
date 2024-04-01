#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

pub use page_table::MapPermission;

pub mod address;
pub mod frame;
pub mod heap;
pub mod page_table;

pub use address::{PhysAddr, PhysPageNum, StepByOne, VPNRange, VirtAddr, VirtPageNum};
pub use frame::{frame_alloc, frame_alloc_contig, frame_dealloc, FrameTracker};
pub use page_table::{PageTable, PageTableEntry};

pub const MMIO: &[(usize, usize, MapPermission)] = &[
    (0x10000000, 0x1000, MapPermission::RW),   // UART
    (0x10001000, 0x1000, MapPermission::RW),   // VIRTIO
    (0x02000000, 0x10000, MapPermission::RW),  // CLINT
    (0x0C000000, 0x400000, MapPermission::RW), // PLIC
];
