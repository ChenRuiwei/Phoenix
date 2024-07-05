#![no_std]
#![no_main]

extern crate alloc;
pub mod error;

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
    /// 随便设的值，Linux中网络设备貌似没有主设备号和从设备号
    Net = 16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DevId {
    /// Major Device Number
    pub major: DeviceMajor,
    /// Minor Device Number. It Identifies different device instances of the
    /// same type
    pub minor: usize,
}

pub struct DeviceMeta {
    /// Device id.
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

pub trait BaseDeviceOps: Sync + Send + DowncastSync {
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

impl_downcast!(sync BaseDeviceOps);

#[async_trait]
pub trait CharDevice: Send + Sync + BaseDeviceOps {
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
