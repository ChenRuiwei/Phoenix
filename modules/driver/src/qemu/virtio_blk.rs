use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    num::NonZeroUsize,
    sync::atomic::{AtomicUsize, Ordering},
};

use config::{
    board::BLOCK_SIZE,
    mm::{MAX_BUFFER_CACHE, MAX_BUFFER_PAGES, VIRT_RAM_OFFSET},
};
use lru::LruCache;
use sync::mutex::{SpinLock, SpinNoIrqLock};
use virtio_drivers::{
    device::blk::VirtIOBlk,
    transport::mmio::{MmioTransport, VirtIOHeader},
};

use super::VirtioHalImpl;
use crate::{
    buffer_cache::{self, BufferCache},
    BlockDevice,
};

pub struct VirtIOBlkDev {
    device: SpinNoIrqLock<VirtIOBlk<VirtioHalImpl, MmioTransport>>,
    cache: SpinNoIrqLock<BufferCache>,
}

unsafe impl Send for VirtIOBlkDev {}
unsafe impl Sync for VirtIOBlkDev {}

impl BlockDevice for VirtIOBlkDev {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        self.cache.lock().read_block(block_id, buf);
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.base_write_block(block_id, buf)
    }

    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }

    fn base_read_block(&self, block_id: usize, buf: &mut [u8]) {
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
        blk_dev.cache.lock().set_device(blk_dev.clone());
        blk_dev
    }
}
