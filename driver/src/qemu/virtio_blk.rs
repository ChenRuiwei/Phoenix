use config::mm::VIRT_RAM_OFFSET;
use sync::mutex::SpinNoIrqLock;
use virtio_drivers::{
    device::blk::VirtIOBlk,
    transport::mmio::{MmioTransport, VirtIOHeader},
};

use super::VirtioHal;
use crate::BlockDevice;

#[allow(unused)]
// const VIRTIO0: usize = 0x10001000;
const VIRTIO0: usize = 0x10001000 + VIRT_RAM_OFFSET;

pub struct VirtIOBlock(SpinNoIrqLock<VirtIOBlk<VirtioHal, MmioTransport>>);

unsafe impl Send for VirtIOBlock {}
unsafe impl Sync for VirtIOBlock {}

impl BlockDevice for VirtIOBlock {
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
impl VirtIOBlock {
    pub fn new() -> Self {
        unsafe {
            let header = &mut *(VIRTIO0 as *mut VirtIOHeader);
            Self(SpinNoIrqLock::new(
                VirtIOBlk::<VirtioHal, MmioTransport>::new(
                    MmioTransport::new(header.into()).unwrap(),
                )
                .unwrap(),
            ))
        }
    }
}
