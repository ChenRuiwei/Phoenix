#![no_std]
#![no_main]

extern crate alloc;

use alloc::{boxed::Box, string::String, sync::Arc};

use async_trait::async_trait;
use downcast_rs::{impl_downcast, DowncastSync};

/// General Device Operations
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DeviceType {
    Block,
    Char,
    Net,
    Display,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
#[repr(usize)]
pub enum DeviceMajor {
    Serial = 4,
    Block = 8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DevId {
    pub major: DeviceMajor,
    pub minor: usize,
}

pub struct DeviceMeta {
    pub dev_id: DevId,
    /// Name of the device.
    pub name: String,
    /// Mmio start address.
    pub mmio_base: usize,
    /// Mmio size.
    pub mmio_size: usize,
    /// Interrupt number.
    pub irq_no: Option<usize>,
    /// Device type.
    pub dtype: DeviceType,
}

pub trait Device: Sync + Send + DowncastSync {
    fn meta(&self) -> &DeviceMeta;

    fn init(&self);

    fn handle_irq(&self);

    fn dev_id(&self) -> DevId {
        self.meta().dev_id
    }

    fn name(&self) -> &str {
        &self.meta().name
    }

    fn mmio_base(&self) -> usize {
        self.meta().mmio_base
    }

    fn mmio_size(&self) -> usize {
        self.meta().mmio_size
    }

    fn irq_no(&self) -> Option<usize> {
        self.meta().irq_no
    }

    fn dtype(&self) -> DeviceType {
        self.meta().dtype
    }
}

impl_downcast!(sync Device);

#[async_trait]
pub trait CharDevice: Send + Sync + Device {
    async fn read(&self, buf: &mut [u8]) -> usize;
    async fn write(&self, buf: &[u8]) -> usize;
    async fn poll_in(&self) -> bool;
    async fn poll_out(&self) -> bool;
}

pub trait BlockDevice: Send + Sync + DowncastSync {
    fn size(&self) -> u64;

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

impl_downcast!(sync BlockDevice);
