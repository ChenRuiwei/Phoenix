use alloc::{string::ToString, sync::Arc};

use config::board::BLOCK_SIZE;
use device_core::{BaseDriverOps, BlockDriverOps, DevId, DeviceMajor, DeviceMeta, DeviceType};
use log::error;
use page::BufferCache;
use sync::mutex::SpinNoIrqLock;
use virtio_drivers::{device::blk::VirtIOBlk, transport::mmio::MmioTransport};

use super::VirtioHalImpl;

pub type BlockDevice = VirtIoBlkDev;

pub struct VirtIoBlkDev {
    meta: DeviceMeta,
    device: SpinNoIrqLock<VirtIOBlk<VirtioHalImpl, MmioTransport>>,
    pub cache: SpinNoIrqLock<BufferCache>,
}

unsafe impl Send for VirtIoBlkDev {}
unsafe impl Sync for VirtIoBlkDev {}

impl BlockDriverOps for VirtIoBlkDev {
    // TODO: cached size value
    fn size(&self) -> u64 {
        self.device.lock().capacity() * (BLOCK_SIZE as u64)
    }

    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }

    fn base_read_block(&self, block_id: usize, buf: &mut [u8]) {
        // log::error!("read blk id {}", block_id);
        let res = self.device.lock().read_blocks(block_id, buf);
        if res.is_err() {
            panic!(
                "Error when reading VirtIOBlk, block_id {} ,err {:?} ",
                block_id, res
            );
        }
    }

    fn base_write_block(&self, block_id: usize, buf: &[u8]) {
        self.device
            .lock()
            .write_blocks(block_id, buf)
            .expect("Error when writing VirtIOBlk");
    }

    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        self.cache.lock().read_block(block_id, buf)
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.cache.lock().write_block(block_id, buf)
    }
}

impl VirtIoBlkDev {
    pub fn try_new(
        mmio_base: usize,
        mmio_size: usize,
        irq_no: usize,
        transport: MmioTransport,
    ) -> Option<Arc<Self>> {
        // const VIRTIO0: usize = 0x10001000 + VIRT_RAM_OFFSET;
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
                    irq_no: None, // TODO: block device don't support interrupts in this system
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

impl BaseDriverOps for VirtIoBlkDev {
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
}