//! Memory management implementation
//!
//! SV39 page-based virtual-memory architecture for RV64 systems, and
//! everything about memory management, like frame allocator, page table,
//! map area and memory set, is implemented here.
//!
//! Every task or process has a memory_space to control its virtual memory.

pub(crate) mod heap_allocator;
///
pub mod memory_space;
mod page;
mod shm;
use config::board::MEMORY_END;
use memory::{frame, VPNRange};
pub use shm::SHARED_MEMORY_MANAGER;
///
pub mod user_check;

use log::info;
pub use memory::page_table::{PageTable, PageTableEntry};
pub use memory_space::{activate_kernel_space, remap_test, MemorySpace, KERNEL_SPACE};
pub use page::{Page, PageBuilder};

use crate::{mm, processor::hart::HARTS};

/// initiate heap allocator, frame allocator and kernel space
pub fn init() {
    extern "C" {
        fn _ekernel();
    }
    heap_allocator::init_heap();
    frame::init_frame_allocator(
        (_ekernel as usize).into(),
        MEMORY_END.into(),
    );
    memory_space::init_kernel_space();
    info!("KERNEL SPACE init finished");
    mm::activate_kernel_space();
    info!("KERNEL SPACE activated");
}
