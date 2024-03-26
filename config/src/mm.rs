pub const RAM_START: usize = 0x8000_0000;
pub const VIRT_START: usize = 0xffff_ffc0_8000_0000;
pub const RAM_SIZE: usize = 128 * 1024 * 1024;

pub const VIRT_RAM_OFFSET: usize = KERNEL_START - KERNEL_START_PHYS;

pub const KERNEL_OFFSET: usize = 0x20_0000;
pub const KERNEL_START_PHYS: usize = RAM_START + KERNEL_OFFSET;
pub const KERNEL_START: usize = VIRT_START + KERNEL_OFFSET;

pub const KERNEL_STACK_SIZE: usize = 4096 * 16;
pub const KERNEL_HEAP_SIZE: usize = 32 * 1024 * 1024;

/// boot
pub const HART_START_ADDR: usize = 0x80200000;

pub const USER_STACK_SIZE: usize = 1024 * 1024 * 8; // 8M

pub const PAGE_SIZE: usize = 1 << PAGE_SIZE_BITS;
pub const PAGE_SIZE_BITS: usize = 12;

/// 3 level for rv39 page table
pub const PAGE_TABLE_LEVEL_NUM: usize = 3;

/// When directly map: vpn = ppn + kernel direct offset
pub const KERNEL_DIRECT_OFFSET: usize = 0xffff_ffc0_0000_0;

pub const USER_SPACE_SIZE: usize = 0x30_0000_0000;

/// Mmap area toppest address
pub const MMAP_TOP: usize = USER_SPACE_SIZE;

/// Dynamic linked interpreter address range in user space
pub const DL_INTERP_OFFSET: usize = 0x20_0000_0000;
