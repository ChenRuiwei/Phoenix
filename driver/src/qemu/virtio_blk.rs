use config::mm::VIRT_RAM_OFFSET;
use sync::mutex::SpinNoIrqLock;
use virtio_drivers::{
    device::blk::VirtIOBlk,
    transport::mmio::{MmioTransport, VirtIOHeader},
};

use super::VirtioHalImpl;
use crate::BlockDevice;

const VIRTIO0: usize = 0x10001000 + VIRT_RAM_OFFSET;

pub struct VirtIOBlkDev(SpinNoIrqLock<VirtIOBlk<VirtioHalImpl, MmioTransport>>);

unsafe impl Send for VirtIOBlkDev {}
unsafe impl Sync for VirtIOBlkDev {}

impl BlockDevice for VirtIOBlkDev {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        let res = self.0.lock().read_blocks(block_id, buf);
        if res.is_err() {
            panic!("Error when reading VirtIOBlk, block_id {}", block_id);
        }
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.0
            .lock()
            .write_blocks(block_id, buf)
            .expect("Error when writing VirtIOBlk");
    }
}

impl VirtIOBlkDev {
    pub fn new() -> Self {
        unsafe {
            let header = &mut *(VIRTIO0 as *mut VirtIOHeader);
            Self(SpinNoIrqLock::new(
                VirtIOBlk::<VirtioHalImpl, MmioTransport>::new(
                    MmioTransport::new(header.into()).unwrap(),
                )
                .unwrap(),
            ))
        }
    }
}
