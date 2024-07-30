use crate::{
    board::{BLOCK_MASK, BLOCK_SIZE},
    utils::register_mut_const,
};

pub const RAM_START: usize = 0x8000_0000;
pub const VIRT_START: usize = 0xffff_ffc0_8000_0000;
pub const RAM_SIZE: usize = 128 * 1024 * 1024;

pub const VIRT_RAM_OFFSET: usize = KERNEL_START - KERNEL_START_PHYS;

pub const KERNEL_OFFSET: usize = 0x20_0000;
pub const KERNEL_START_PHYS: usize = RAM_START + KERNEL_OFFSET;
pub const KERNEL_START: usize = VIRT_START + KERNEL_OFFSET;

pub const KERNEL_STACK_SIZE: usize = 64 * 1024;
pub const KERNEL_HEAP_SIZE: usize = 56 * 1024 * 1024;

pub const HART_START_ADDR: usize = 0x80200000;

register_mut_const!(pub DTB_ADDR, usize, 0);

pub const USER_ELF_PRE_ALLOC_PAGE_CNT: usize = 0;

pub const PAGE_SIZE: usize = 1 << PAGE_SIZE_BITS;
pub const PAGE_MASK: usize = PAGE_SIZE - 1;
pub const PAGE_SIZE_BITS: usize = 12;

pub const PTE_SIZE: usize = 8;
pub const PTES_PER_PAGE: usize = PAGE_SIZE / PTE_SIZE;

/// 3 level for sv39 page table
pub const PAGE_TABLE_LEVEL_NUM: usize = 3;

pub const MMAP_PRE_ALLOC_PAGES: usize = 8;

/// Dynamic linked interpreter address range in user space
pub const DL_INTERP_OFFSET: usize = 0x20_0000_0000;

pub const MAX_BUFFER_HEADS: usize = 0x18000;
pub const MAX_BUFFER_CACHE: usize = 0x1000;
pub const MAX_BUFFER_PAGES: usize = MAX_BUFFER_CACHE / MAX_BUFFERS_PER_PAGE;
pub const MAX_BUFFERS_PER_PAGE: usize = PAGE_SIZE / BLOCK_SIZE;
pub const BUFFER_NEED_CACHE_CNT: usize = 8;

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

// =========== Kernel segments ===========
pub const K_SEG_BEG: usize = 0xffff_ffc0_0000_0000;

// Kernel heap segment (64 GiB)
pub const K_SEG_HEAP_BEG: usize = 0xffff_ffc0_0000_0000;
pub const K_SEG_HEAP_END: usize = 0xffff_ffd0_0000_0000;

// File mapping (64 GiB)
pub const K_SEG_FILE_BEG: usize = 0xffff_ffd0_0000_0000;
pub const K_SEG_FILE_END: usize = 0xffff_ffe0_0000_0000;

// Physical memory mapping segment (62 GiB)
pub const K_SEG_PHY_MEM_BEG: usize = 0xffff_fff0_0000_0000;
pub const K_SEG_PHY_MEM_END: usize = 0xffff_ffff_8000_0000;

// Kernel text segment (1 GiB)
pub const K_SEG_TEXT_BEG: usize = 0xffff_ffff_8000_0000;
pub const K_SEG_TEXT_END: usize = 0xffff_ffff_c000_0000;

// DTB fixed mapping
pub const K_SEG_DTB_BEG: usize = K_SEG_DTB_END - MAX_DTB_SIZE;
pub const K_SEG_DTB_END: usize = 0xffff_ffff_f000_0000;
pub const MAX_DTB_SIZE: usize = PAGE_SIZE * PAGE_SIZE;

pub fn align_offset_to_page(offset: usize) -> (usize, usize) {
    let offset_aligned = offset & !PAGE_MASK;
    let offset_in_page = offset - offset_aligned;
    (offset_aligned, offset_in_page)
}

pub fn is_aligned_to_page(offset: usize) -> bool {
    offset & PAGE_MASK == 0
}

pub fn is_aligned_to_block(offset: usize) -> bool {
    offset & BLOCK_MASK == 0
}

pub fn round_down_to_page(offset: usize) -> usize {
    offset & !PAGE_MASK
}

pub fn round_up_to_page(offset: usize) -> usize {
    round_down_to_page(offset + PAGE_MASK)
}

pub fn block_page_id(block_id: usize) -> usize {
    block_id / MAX_BUFFERS_PER_PAGE
}

pub fn block_page_offset(block_id: usize) -> usize {
    (block_id % MAX_BUFFERS_PER_PAGE) * BLOCK_SIZE
}
