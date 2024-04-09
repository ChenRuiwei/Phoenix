//! Memory management implementation
//!
//! SV39 page-based virtual-memory architecture for RV64 systems, and
//! everything about memory management, like frame allocator, page table,
//! map area and memory set, is implemented here.
//!
//! Every task or process has a memory_space to control its virtual memory.

///
pub mod memory_space;
mod page;
mod user_ptr;

use config::board::MEMORY_END;
use log::info;
pub use memory::page_table::PageTable;
use memory::{frame, heap, VirtAddr};
pub use memory_space::{activate_kernel_space, MemorySpace};
pub use page::Page;
pub use user_ptr::{UserInOutPtr, UserReadPtr, UserWritePtr};

use self::memory_space::vm_area::MapPermission;
use crate::mm;

/// initiate heap allocator, frame allocator and kernel space
pub fn init() {
    extern "C" {
        fn _ekernel();
    }
    heap::init_heap_allocator();
    frame::init_frame_allocator(
        VirtAddr::from(_ekernel as usize).to_offset().to_pa().into(),
        VirtAddr::from(MEMORY_END).to_offset().to_pa().into(),
    );
    info!("KERNEL SPACE init finished");
    unsafe { mm::activate_kernel_space() };
    info!("KERNEL SPACE activated");
}

pub const MMIO: &[(usize, usize, MapPermission)] = &[
    (0x10000000, 0x1000, MapPermission::RW),   // UART
    (0x10001000, 0x1000, MapPermission::RW),   // VIRTIO
    (0x02000000, 0x10000, MapPermission::RW),  // CLINT
    (0x0C000000, 0x400000, MapPermission::RW), // PLIC
];
