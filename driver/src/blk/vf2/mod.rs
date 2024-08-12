use alloc::{string::ToString, sync::Arc};

use arch::time::get_time;
use config::board::{clock_freq, BLOCK_SIZE};
use device_core::{BlockDevice, DevId, Device, DeviceMajor, DeviceMeta, DeviceType};
use memory::PhysAddr;
use sync::mutex::SpinNoIrqLock;
use visionfive2_sd::{SDIo, SleepOps, Vf2SdDriver};

pub fn sleep_ms(ms: usize) {
    let start = get_time();
    while get_time() - start < ms * (clock_freq() / 1000) {
        core::hint::spin_loop();
    }
}

pub fn sleep_ms_until(ms: usize, mut f: impl FnMut() -> bool) {
    let start = get_time();
    while get_time() - start < ms * (clock_freq() / 1000) {
        if f() {
            return;
        }
        core::hint::spin_loop();
    }
}
pub struct SdIoImpl;
pub const SDIO_BASE: usize = 0x16020000;
impl SDIo for SdIoImpl {
    fn read_data_at(&self, offset: usize) -> u64 {
        let addr = PhysAddr::from(SDIO_BASE + offset).to_vaddr().bits() as *mut u64;
        unsafe { addr.read_volatile() }
    }
    fn read_reg_at(&self, offset: usize) -> u32 {
        let addr = PhysAddr::from(SDIO_BASE + offset).to_vaddr().bits() as *mut u32;
        unsafe { addr.read_volatile() }
    }
    fn write_data_at(&mut self, offset: usize, val: u64) {
        let addr = PhysAddr::from(SDIO_BASE + offset).to_vaddr().bits() as *mut u64;
        unsafe { addr.write_volatile(val) }
    }
    fn write_reg_at(&mut self, offset: usize, val: u32) {
        let addr = PhysAddr::from(SDIO_BASE + offset).to_vaddr().bits() as *mut u32;
        unsafe { addr.write_volatile(val) }
    }
}

pub struct SleepOpsImpl;

impl SleepOps for SleepOpsImpl {
    fn sleep_ms(ms: usize) {
        sleep_ms(ms)
    }
    fn sleep_ms_until(ms: usize, f: impl FnMut() -> bool) {
        sleep_ms_until(ms, f)
    }
}

pub struct Vf2SDImpl {
    meta: DeviceMeta,
    driver: SpinNoIrqLock<Vf2SdDriver<SdIoImpl, SleepOpsImpl>>,
}

impl Vf2SDImpl {
    pub fn new(mmio_base: usize, mmio_size: usize, _irq_no: Option<usize>) -> Arc<Vf2SDImpl> {
        let meta = DeviceMeta {
            dev_id: DevId {
                major: DeviceMajor::Block,
                minor: 0,
            },
            name: "vfs-blk".to_string(),
            mmio_base,
            mmio_size,
            irq_no: None, // TODO: support interrupt for block device
            dtype: DeviceType::Block,
        };

        let driver = SpinNoIrqLock::new(Vf2SdDriver::new(SdIoImpl));
        Arc::new(Self { meta, driver })
    }
}

impl Device for Vf2SDImpl {
    fn meta(&self) -> &DeviceMeta {
        &self.meta
    }

    fn init(&self) {
        self.driver.lock().init()
    }

    fn handle_irq(&self) {
        todo!()
    }

    fn as_blk(self: Arc<Self>) -> Option<Arc<dyn BlockDevice>> {
        Some(self)
    }
}

impl BlockDevice for Vf2SDImpl {
    fn size(&self) -> u64 {
        16 * 1024 * 1024 * 1024
    }

    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }

    fn buffer_head_cnts(&self) -> usize {
        todo!()
    }

    fn remove_buffer_page(&self, block_id: usize) {
        todo!()
    }

    fn base_read_blocks(&self, block_id: usize, buf: &mut [u8]) {
        self.driver.lock().read_block(block_id, buf)
    }

    fn base_write_blocks(&self, block_id: usize, buf: &[u8]) {
        // log::error!("base write block {block_id}");
        // self.driver.lock().write_block(block_id, buf)
    }

    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        self.base_read_blocks(block_id, buf)
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.base_write_blocks(block_id, buf)
    }
}
