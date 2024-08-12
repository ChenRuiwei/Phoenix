// use device_core::DeviceMeta;
// use embedded_sdmmc::SdCard;
// use sync::mutex::SpinNoIrqLock;
//
// pub struct SDImpl {
//     meta: DeviceMeta,
//     driver: SpinNoIrqLock<SdCard>,
// }
//
// impl SDImpl {
//     pub fn new(mmio_base: usize, mmio_size: usize, _irq_no: Option<usize>) ->
// Arc<Vf2SDImpl> {         let meta = DeviceMeta {
//             dev_id: DevId {
//                 major: DeviceMajor::Block,
//                 minor: 0,
//             },
//             name: "vfs-blk".to_string(),
//             mmio_base,
//             mmio_size,
//             irq_no: None, // TODO: support interrupt for block device
//             dtype: DeviceType::Block,
//         };
//
//         let sdcard = embedded_sdmmc::SdCard::new(sdmmc_spi, delay);
//         let driver = SpinNoIrqLock::new(Vf2SdDriver::new(SdIoImpl));
//         Arc::new(Self { meta, driver })
//     }
// }
//
// impl Device for Vf2SDImpl {
//     fn meta(&self) -> &DeviceMeta {
//         &self.meta
//     }
//
//     fn init(&self) {
//         self.driver.lock().init()
//     }
//
//     fn handle_irq(&self) {
//         todo!()
//     }
//
//     fn as_blk(self: Arc<Self>) -> Option<Arc<dyn BlockDevice>> {
//         Some(self)
//     }
// }
//
// impl BlockDevice for Vf2SDImpl {
//     fn size(&self) -> u64 {
//         16 * 1024 * 1024 * 1024
//     }
//
//     fn block_size(&self) -> usize {
//         BLOCK_SIZE
//     }
//
//     fn buffer_head_cnts(&self) -> usize {
//         todo!()
//     }
//
//     fn remove_buffer_page(&self, block_id: usize) {
//         todo!()
//     }
//
//     fn base_read_blocks(&self, block_id: usize, buf: &mut [u8]) {
//         self.driver.lock().read_block(block_id, buf)
//     }
//
//     fn base_write_blocks(&self, block_id: usize, buf: &[u8]) {
//         self.driver.lock().write_block(block_id, buf)
//     }
//
//     fn read_block(&self, block_id: usize, buf: &mut [u8]) {
//         self.driver.lock().read_block(block_id, buf)
//     }
//
//     fn write_block(&self, block_id: usize, buf: &[u8]) {
//         self.driver.lock().write_block(block_id, buf)
//     }
// }
