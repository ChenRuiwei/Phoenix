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
pub use memory::page_table::PageTable;
use memory::{frame, heap, VirtAddr};
pub use memory_space::{switch_kernel_page_table, MemorySpace};
pub use page::Page;
pub use user_ptr::{FutexWord, UserReadPtr, UserWritePtr};

use self::memory_space::vm_area::MapPerm;
use crate::mm;

/// Initialize heap allocator, frame allocator and kernel space.
pub fn init() {
    extern "C" {
        fn _ekernel();
    }
    heap::init_heap_allocator();
    frame::init_frame_allocator(
        VirtAddr::from(_ekernel as usize).to_offset().to_pa().into(),
        VirtAddr::from(MEMORY_END).to_offset().to_pa().into(),
    );
    unsafe { mm::switch_kernel_page_table() };
    log::info!("KERNEL SPACE activated");
}

/// MMIO in QEMU
pub const MMIO: &[(usize, usize, MapPerm)] = &[
    (0x10000000, 0x1000, MapPerm::RW),   // UART
    (0x10001000, 0x1000, MapPerm::RW),   // VIRTIO
    (0x02000000, 0x10000, MapPerm::RW),  // CLINT
    (0x0C000000, 0x400000, MapPerm::RW), // PLIC
];
