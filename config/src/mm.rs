pub const RAM_START: usize = 0x8000_0000;
pub const VIRT_START: usize = 0xffff_ffc0_8000_0000;
pub const RAM_SIZE: usize = 128 * 1024 * 1024;

pub const VIRT_RAM_OFFSET: usize = KERNEL_START - KERNEL_START_PHYS;

pub const KERNEL_OFFSET: usize = 0x20_0000;
pub const KERNEL_START_PHYS: usize = RAM_START + KERNEL_OFFSET;
pub const KERNEL_START: usize = VIRT_START + KERNEL_OFFSET;

pub const KERNEL_STACK_SIZE: usize = 4096 * 16; // 64M
pub const KERNEL_HEAP_SIZE: usize = 32 * 1024 * 1024; // 32M

/// boot
pub const HART_START_ADDR: usize = 0x80200000;

pub const USER_STACK_SIZE: usize = 1024 * 1024 * 8; // 8M

pub const PAGE_SIZE: usize = 1 << PAGE_SIZE_BITS;
pub const PAGE_MASK: usize = PAGE_SIZE - 1;
pub const PAGE_SIZE_BITS: usize = 12;
pub const PTE_NUM_ONE_PAGE: usize = 512;

/// 3 level for sv39 page table
pub const PAGE_TABLE_LEVEL_NUM: usize = 3;
