#![no_std]
#![no_main]

extern crate alloc;

pub use page_table::MapPermission;

pub mod address;
pub mod frame_allocator;
pub mod page_table;

pub use address::{PhysAddr, PhysPageNum, StepByOne, VPNRange, VirtAddr, VirtPageNum};
pub use frame_allocator::{frame_alloc, frame_alloc_contig, frame_dealloc, FrameTracker};
pub use page_table::{PageTable, PageTableEntry};

pub const PERMISSION_RW: MapPermission = MapPermission::union(MapPermission::R, MapPermission::W);

pub const MMIO: &[(usize, usize, MapPermission)] = &[
    (0x10000000, 0x1000, PERMISSION_RW),   // UART
    (0x10001000, 0x1000, PERMISSION_RW),   // VIRTIO
    (0x02000000, 0x10000, PERMISSION_RW),  // CLINT
    (0x0C000000, 0x400000, PERMISSION_RW), // PLIC
];
