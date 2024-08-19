mod dw_mshc;
mod vf2;
mod virtio;

use alloc::sync::Arc;

use device_core::DeviceType;
use fdt::Fdt;
use memory::{pte::PTEFlags, PhysAddr};
pub use virtio::*;
use visionfive2_sd::Vf2SdDriver;

use self::dw_mshc::MMC;
use super::wait_for;
use crate::{blk::vf2::Vf2SDImpl, kernel_page_table_mut, virtio::probe_devices_common};

pub fn probe_sdio_blk(root: &Fdt) -> Option<Arc<MMC>> {
    // Parse SD Card Host Controller
    if let Some(sdhci) = root.find_node("/soc/sdio1@16020000") {
        let base_address = sdhci.reg().unwrap().next().unwrap().starting_address as usize;
        let size = sdhci.reg().unwrap().next().unwrap().size.unwrap();
        let irq_number = 33; // Hard-coded from JH7110
        let sdcard = MMC::new(base_address, size, irq_number);
        log::info!("SD Card Host Controller found at 0x{:x}", base_address);
        return Some(Arc::new(sdcard));
    }
    log::warn!("SD Card Host Controller not found");
    None
}

pub fn probe_virtio_blk(root: &Fdt) -> Option<Arc<VirtIoBlkDev>> {
    let device_tree = root;
    let mut dev = None;
    for node in device_tree.find_all_nodes("/soc/virtio_mmio") {
        for reg in node.reg()? {
            let mmio_base_paddr = PhysAddr::from(reg.starting_address as usize);
            let mmio_size = reg.size?;
            let irq_no = node.property("interrupts").and_then(|i| i.as_usize());
            // First map memory, probe virtio device need to map it
            kernel_page_table_mut().ioremap(
                mmio_base_paddr.bits(),
                mmio_size,
                PTEFlags::R | PTEFlags::W,
            );
            dev = probe_devices_common(DeviceType::Block, mmio_base_paddr, mmio_size, |t| {
                VirtIoBlkDev::try_new(mmio_base_paddr.bits(), mmio_size, irq_no, t)
            });
            kernel_page_table_mut().iounmap(mmio_base_paddr.to_vaddr().bits(), mmio_size);
            if dev.is_some() {
                break;
            }
        }
    }
    if dev.is_none() {
        log::warn!("No virtio block device found");
    }
    dev
}

pub fn probe_vf2_sd(root: &Fdt) -> Option<Arc<Vf2SDImpl>> {
    // Parse SD Card Host Controller
    if let Some(sdhci) = root.find_node("/soc/sdio1@16020000") {
        let base_address = sdhci.reg().unwrap().next().unwrap().starting_address as usize;
        let size = sdhci.reg().unwrap().next().unwrap().size.unwrap();
        let irq_number = Some(33); // Hard-coded from JH7110         let sdcard =
        let sdcard = Vf2SDImpl::new(base_address, size, irq_number);
        log::info!(
            "SD Card
     Host Controller found at 0x{:x}",
            base_address
        );
        return Some(sdcard);
    }
    log::warn!("SD Card Host Controller not found");
    None
}
