pub const RAM_START: usize = 0x8000_0000;
pub const VIRT_START: usize = 0xffff_ffc0_8000_0000;
pub const RAM_SIZE: usize = 128 * 1024 * 1024;

pub const VIRT_RAM_OFFSET: usize = KERNEL_START - KERNEL_START_PHYS;

pub const KERNEL_OFFSET: usize = 0x20_0000;
pub const KERNEL_START_PHYS: usize = RAM_START + KERNEL_OFFSET;
pub const KERNEL_START: usize = VIRT_START + KERNEL_OFFSET;

pub const KERNEL_STACK_SIZE: usize = 64 * 1024; // 64K
pub const KERNEL_HEAP_SIZE: usize = 32 * 1024 * 1024; // 32M

/// boot
pub const HART_START_ADDR: usize = 0x80200000;

pub const USER_STACK_SIZE: usize = 8 * 1024 * 1024; // 8M

pub const PAGE_SIZE: usize = 1 << PAGE_SIZE_BITS;
pub const PAGE_MASK: usize = PAGE_SIZE - 1;
pub const PAGE_SIZE_BITS: usize = 12;

pub const PTE_SIZE: usize = 8;
pub const PTE_NUM_IN_ONE_PAGE: usize = PAGE_SIZE / PTE_SIZE;

/// 3 level for sv39 page table
pub const PAGE_TABLE_LEVEL_NUM: usize = 3;

/// Dynamic linked interpreter address range in user space
pub const DL_INTERP_OFFSET: usize = 0x20_0000_0000;

/// User stack segment
pub const U_SEG_STACK_BEG: usize = 0x0000_0001_0000_0000;
pub const U_SEG_STACK_END: usize = 0x0000_0002_0000_0000;

/// User heap segment
// pub const U_SEG_HEAP_BEG: usize = 0x0000_0002_0000_0000;
// pub const U_SEG_HEAP_END: usize = 0x0000_0004_0000_0000;
pub const U_SEG_HEAP_BEG: usize = 0x0000_0000_4000_0000;
pub const U_SEG_HEAP_END: usize = 0x0000_0000_8000_0000;

/// User mmap segment
pub const U_SEG_FILE_BEG: usize = 0x0000_0004_0000_0000;
pub const U_SEG_FILE_END: usize = 0x0000_0006_0000_0000;

/// User share segment
pub const U_SEG_SHARE_BEG: usize = 0x0000_0006_0000_0000;
pub const U_SEG_SHARE_END: usize = 0x0000_0008_0000_0000;
