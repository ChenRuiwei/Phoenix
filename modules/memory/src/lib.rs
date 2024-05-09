#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(const_trait_impl)]
#![feature(stdsimd)]
#![feature(riscv_ext_intrinsics)]
#![feature(step_trait)]

extern crate alloc;

pub mod address;
pub mod frame;
pub mod heap;
pub mod page;
pub mod page_table;
pub mod pte;

pub use address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
pub use frame::{alloc_frame, alloc_frames, dealloc_frame, FrameTracker};
pub use page_table::PageTable;
pub use pte::PageTableEntry;
