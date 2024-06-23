use alloc::sync::Arc;

use config::{board::BLOCK_SIZE, mm::VIRT_RAM_OFFSET};
use device_core::BlockDevice;
use page::BufferCache;
use sync::mutex::SpinNoIrqLock;
use virtio_drivers::{
    device::blk::VirtIOBlk,
    transport::mmio::{MmioTransport, VirtIOHeader},
};

use super::VirtioHalImpl;

pub struct VirtIOBlkDev {
    device: SpinNoIrqLock<VirtIOBlk<VirtioHalImpl, MmioTransport>>,
    pub cache: SpinNoIrqLock<BufferCache>,
}

unsafe impl Send for VirtIOBlkDev {}
unsafe impl Sync for VirtIOBlkDev {}

impl BlockDevice for VirtIOBlkDev {
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

impl VirtIOBlkDev {
    pub fn new() -> Arc<Self> {
        const VIRTIO0: usize = 0x10001000 + VIRT_RAM_OFFSET;
        let device = unsafe {
            let header = &mut *(VIRTIO0 as *mut VirtIOHeader);
            SpinNoIrqLock::new(
                VirtIOBlk::<VirtioHalImpl, MmioTransport>::new(
                    MmioTransport::new(header.into()).unwrap(),
                )
                .unwrap(),
            )
        };
        let blk_dev = Arc::new(Self {
            device,
            cache: SpinNoIrqLock::new(BufferCache::new()),
        });
        blk_dev.cache.lock().init_device(blk_dev.clone());
        blk_dev
    }
}
