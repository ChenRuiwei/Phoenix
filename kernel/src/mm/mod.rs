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

use config::board::MEMORY_END;
use log::info;
pub use memory::page_table::{PageTable, PageTableEntry};
use memory::{frame, heap, VPNRange};
pub use memory_space::{activate_kernel_space, MemorySpace, KERNEL_SPACE};
pub use page::Page;

use crate::{mm, processor::hart::HARTS};

/// initiate heap allocator, frame allocator and kernel space
pub fn init() {
    extern "C" {
        fn _ekernel();
    }
    heap::init_heap_allocator();
    frame::init_frame_allocator((_ekernel as usize).into(), MEMORY_END.into());
    info!("KERNEL SPACE init finished");
    mm::activate_kernel_space();
    info!("KERNEL SPACE activated");
}
