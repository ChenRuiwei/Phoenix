use alloc::{string::ToString, sync::Arc};
use core::ptr::NonNull;

use config::board::BLOCK_SIZE;
use device_core::{BlockDevice, DevId, Device, DeviceMajor, DeviceMeta, DeviceType};
use log::error;
use memory::{alloc_frames, dealloc_frame, PhysAddr, PhysPageNum, VirtAddr};
use page::BufferCache;
use sync::mutex::SpinNoIrqLock;
use virtio_drivers::{device::blk::VirtIOBlk, transport::mmio::MmioTransport, BufferDirection};

use crate::virtio::VirtioHalImpl;

pub type BlockDeviceImpl = VirtIoBlkDev;

pub struct VirtIoBlkDev {
    meta: DeviceMeta,
    device: SpinNoIrqLock<VirtIOBlk<VirtioHalImpl, MmioTransport>>,
    pub cache: SpinNoIrqLock<BufferCache>,
}

unsafe impl Send for VirtIoBlkDev {}
unsafe impl Sync for VirtIoBlkDev {}

impl BlockDevice for VirtIoBlkDev {
    // TODO: cached size value
    fn size(&self) -> u64 {
        self.device.lock().capacity() * (BLOCK_SIZE as u64)
    }

    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }

    fn buffer_head_cnts(&self) -> usize {
        self.cache.lock().buffer_heads.len()
    }

    fn remove_buffer_page(&self, block_id: usize) {
        self.cache.lock().pages.pop(&block_id);
    }

    fn base_read_blocks(&self, block_id: usize, buf: &mut [u8]) {
        let res = self.device.lock().read_blocks(block_id, buf);
        if res.is_err() {
            panic!(
                "Error when reading VirtIOBlk, block_id {} ,err {:?} ",
                block_id, res
            );
        }
        // log::warn!("read buf {buf:?}");
        // log::error!("read hash value {}", exam_hash(buf));
    }

    fn base_write_blocks(&self, block_id: usize, buf: &[u8]) {
        self.device
            .lock()
            .write_blocks(block_id, buf)
            .expect("Error when writing VirtIOBlk");

        // log::warn!("write buf {buf:?}");
        // log::error!("write hash value {}", exam_hash(buf));
    }

    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        let res = self.device.lock().read_blocks(block_id, buf);
        if res.is_err() {
            panic!(
                "Error when reading VirtIOBlk, block_id {} ,err {:?} ",
                block_id, res
            );
        }
        // self.cache.lock().read_block(block_id, buf)
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.device
            .lock()
            .write_blocks(block_id, buf)
            .expect("Error when writing VirtIOBlk");
        // self.cache.lock().write_block(block_id, buf)
    }
}

impl VirtIoBlkDev {
    pub fn try_new(
        mmio_base: usize,
        mmio_size: usize,
        _irq_no: Option<usize>,
        transport: MmioTransport,
    ) -> Option<Arc<Self>> {
        match VirtIOBlk::<VirtioHalImpl, MmioTransport>::new(transport) {
            Ok(virtio_blk) => {
                let device = SpinNoIrqLock::new(virtio_blk);
                let meta = DeviceMeta {
                    dev_id: DevId {
                        major: DeviceMajor::Block,
                        minor: 0,
                    },
                    name: "virtio-blk".to_string(),
                    mmio_base,
                    mmio_size,
                    irq_no: None, // TODO: support interrupt for block device
                    dtype: DeviceType::Block,
                };
                let blk_dev = Arc::new(Self {
                    meta,
                    device,
                    cache: SpinNoIrqLock::new(BufferCache::new()),
                });
                blk_dev.cache.lock().init_device(blk_dev.clone());
                Some(blk_dev)
            }
            Err(e) => {
                error!(
                    "[virtio-blk] failed to initialize MMIO device at [PA:{:#x}, PA:{:#x}), {e:?}",
                    mmio_base,
                    mmio_base + mmio_size
                );
                None
            }
        }
    }
}

impl Device for VirtIoBlkDev {
    fn meta(&self) -> &device_core::DeviceMeta {
        &self.meta
    }

    fn init(&self) {
        // let transport = unsafe { MmioTransport::new(header) }.ok()?;
        // self.device.try_new(transport);
    }

    fn handle_irq(&self) {
        // todo!()
    }

    fn as_blk(self: Arc<Self>) -> Option<Arc<dyn BlockDevice>> {
        Some(self)
    }
}
