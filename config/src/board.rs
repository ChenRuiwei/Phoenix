use crate::{
    mm::{RAM_SIZE, VIRT_START},
    utils::register_mut_const,
};

pub const BLOCK_SIZE: usize = 512;
pub const BLOCK_MASK: usize = BLOCK_SIZE - 1;
pub const MEMORY_END: usize = VIRT_START + RAM_SIZE;

pub const UART_BUF_LEN: usize = 512;

pub const MAX_HARTS: usize = 4;
register_mut_const!(pub HARTS, usize, 1);
register_mut_const!(pub CLOCK_FREQ, usize, 10000000);
