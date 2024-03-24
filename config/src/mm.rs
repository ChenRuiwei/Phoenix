pub const RAM_START: usize = 0x8000_0000;
pub const VIRT_START: usize = 0xffff_ffc0_8000_0000;
pub const RAM_SIZE: usize = 128 * 1024 * 1024;

pub const VIRT_RAM_OFFSET: usize = KERNEL_START - KERNEL_START_PHYS;

pub const KERNEL_OFFSET: usize = 0x20_0000;
pub const KERNEL_START_PHYS: usize = RAM_START + KERNEL_OFFSET;
pub const KERNEL_START: usize = VIRT_START + KERNEL_OFFSET;

pub const KERNEL_HEAP_SIZE: usize = 32 * 1024 * 1024;
