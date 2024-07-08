#![no_std]
#![no_main]

extern crate alloc;
pub mod error;

use alloc::{boxed::Box, string::String, sync::Arc};
use core::ptr::NonNull;

use async_trait::async_trait;
use downcast_rs::{impl_downcast, DowncastSync};
use error::DevResult;

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

pub trait BaseDriverOps: Sync + Send + DowncastSync {
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

impl_downcast!(sync BaseDriverOps);

#[async_trait]
pub trait CharDevice: BaseDriverOps {
    async fn read(&self, buf: &mut [u8]) -> usize;
    async fn write(&self, buf: &[u8]) -> usize;
    async fn poll_in(&self) -> bool;
    async fn poll_out(&self) -> bool;
}

pub trait BlockDriverOps: BaseDriverOps {
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

impl_downcast!(sync BlockDriverOps);

/// The ethernet address of the NIC (MAC address).
pub struct EthernetAddress(pub [u8; 6]);

/// Every Net Device should implement this trait
pub trait NetDriverOps: Sync + Send {
    /// The ethernet address of the NIC.
    fn mac_address(&self) -> EthernetAddress;

    /// Whether can transmit packets.
    fn can_transmit(&self) -> bool;

    /// Whether can receive packets.
    fn can_receive(&self) -> bool;

    /// Size of the receive queue.
    fn rx_queue_size(&self) -> usize;

    /// Size of the transmit queue.
    fn tx_queue_size(&self) -> usize;

    /// Gives back the `rx_buf` to the receive queue for later receiving.
    ///
    /// `rx_buf` should be the same as the one returned by
    /// [`NetDriverOps::receive`].
    fn recycle_rx_buffer(&mut self, rx_buf: NetBufPtr) -> DevResult;

    /// Poll the transmit queue and gives back the buffers for previous
    /// transmiting. returns [`DevResult`].
    fn recycle_tx_buffers(&mut self) -> DevResult;

    /// Transmits a packet in the buffer to the network, without blocking,
    /// returns [`DevResult`].
    fn transmit(&mut self, tx_buf: NetBufPtr) -> DevResult;

    /// Receives a packet from the network and store it in the [`NetBuf`],
    /// returns the buffer.
    ///
    /// Before receiving, the driver should have already populated some buffers
    /// in the receive queue by [`NetDriverOps::recycle_rx_buffer`].
    ///
    /// If currently no incomming packets, returns an error with type
    /// [`DevError::Again`].
    fn receive(&mut self) -> DevResult<NetBufPtr>;

    /// Allocate a memory buffer of a specified size for network transmission,
    /// returns [`DevResult`]
    fn alloc_tx_buffer(&mut self, size: usize) -> DevResult<NetBufPtr>;
}

/// A raw buffer struct for network device.
pub struct NetBufPtr {
    // The raw pointer of the original object.
    raw_ptr: NonNull<u8>,
    // The pointer to the net buffer.
    buf_ptr: NonNull<u8>,
    len: usize,
}

impl NetBufPtr {
    /// Create a new [`NetBufPtr`].
    pub fn new(raw_ptr: NonNull<u8>, buf_ptr: NonNull<u8>, len: usize) -> Self {
        Self {
            raw_ptr,
            buf_ptr,
            len,
        }
    }

    /// Return raw pointer of the original object.
    pub fn raw_ptr<T>(&self) -> *mut T {
        self.raw_ptr.as_ptr() as *mut T
    }

    /// Return [`NetBufPtr`] buffer len.
    pub fn packet_len(&self) -> usize {
        self.len
    }

    /// Return [`NetBufPtr`] buffer as &[u8].
    pub fn packet(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.buf_ptr.as_ptr() as *const u8, self.len) }
    }

    /// Return [`NetBufPtr`] buffer as &mut [u8].
    pub fn packet_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.buf_ptr.as_ptr(), self.len) }
    }
}
