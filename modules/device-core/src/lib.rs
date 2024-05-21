#![no_std]
#![no_main]

use core::task::Waker;

pub trait BlockDevice: Send + Sync {
    fn block_size(&self) -> usize;

    /// Read data form block to buffer
    fn base_read_block(&self, block_id: usize, buf: &mut [u8]);

    /// Write data from buffer to block
    fn base_write_block(&self, block_id: usize, buf: &[u8]);

    /// Read data form block to buffer
    fn read_block(&self, block_id: usize, buf: &mut [u8]);

    /// Write data from buffer to block
    fn write_block(&self, block_id: usize, buf: &[u8]);
}

pub trait CharDevice: Send + Sync {
    fn getchar(&self) -> u8;
    fn puts(&self, char: &[u8]);
    fn handle_irq(&self);
    fn register_waker(&self, _waker: Waker) {
        todo!()
    }
}
