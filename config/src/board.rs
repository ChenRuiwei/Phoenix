#![allow(dead_code)]
use crate::mm::{KERNEL_DIRECT_OFFSET, PAGE_SIZE_BITS};

pub const BLOCK_SIZE: usize = 512;
pub const CLOCK_FREQ: usize = 10000000;
pub const MEMORY_END: usize = (KERNEL_DIRECT_OFFSET << PAGE_SIZE_BITS) + 0x88000000;
