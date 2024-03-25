#![allow(dead_code)]
use crate::mm::VIRT_RAM_OFFSET;

pub const BLOCK_SIZE: usize = 512;
pub const CLOCK_FREQ: usize = 10000000;
pub const MEMORY_END: usize = VIRT_RAM_OFFSET + 0x88000000;
