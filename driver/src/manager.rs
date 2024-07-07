//! Device manager

use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};

use arch::interrupts::{disable_interrupt, enable_external_interrupt};
use config::{
    mm::{DTB_ADDR, VIRT_RAM_OFFSET},
    processor::HART_NUM,
};
use device_core::{BaseDeviceOps, DevId};
use log::{info, warn};
use memory::pte::PTEFlags;
use spin::Once;

use super::{plic, CharDevice};
use crate::{
    cpu::{self, CPU},
    kernel_page_table,
    plic::PLIC,
    println,
    qemu::virtio_net::{self, NetDevice, VirtIoNet},
    serial,
};

// pub enum DeviceEnum {
//     /// Network card device.
//     Net(VirtIoNet),
//     // Block storage device.
//     // Block(AxBlockDevice),
//     // Display(AxDisplayDevice),
// }

pub struct DeviceManager {
    plic: Option<PLIC>,
    cpus: Vec<CPU>,
    // blk: Vec<Arc<BlockDevice>>,
    // net: Vec<Arc<NetDriverOps>>,
    pub devices: BTreeMap<DevId, Arc<dyn BaseDeviceOps>>,
    /// irq_no -> device.
    pub irq_map: BTreeMap<usize, Arc<dyn BaseDeviceOps>>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            plic: None,
            cpus: Vec::new(),
            devices: BTreeMap::new(),
            irq_map: BTreeMap::new(),
        }
    }

    pub fn probe(&mut self) {
        let device_tree = unsafe {
            fdt::Fdt::from_ptr((DTB_ADDR + VIRT_RAM_OFFSET) as _).expect("Parse DTB failed")
        };
        let chosen = device_tree.chosen();
        if let Some(bootargs) = chosen.bootargs() {
            println!("Bootargs: {:?}", bootargs);
        }
        println!("Device: {}", device_tree.root().model());

        // Probe PLIC
        self.plic = plic::probe(&device_tree);

        // Probe serial console
        let char_device = Arc::new(self.probe_char_device(&device_tree).unwrap());
        self.devices
            .insert(char_device.dev_id(), char_device.clone());

        self.cpus.extend(cpu::probe());

        self.probe_virtio_device(&device_tree);

        // Add to interrupt map if have interrupts
        for dev in self.devices.values() {
            if let Some(irq) = dev.irq_no() {
                self.irq_map.insert(irq, dev.clone());
            }
        }
    }
    pub fn init_devices(&mut self) {
        for dev in self.devices.values() {
            dev.init();
        }
    }

    pub fn map_devices(&self) {
        let kpt = kernel_page_table();
        for dev in self.devices.values() {
            kpt.ioremap(dev.mmio_base(), dev.mmio_size(), PTEFlags::R | PTEFlags::W);
        }
        let plic = self.plic();
        kpt.ioremap(plic.mmio_base, plic.mmio_size, PTEFlags::R | PTEFlags::W)
    }

    fn plic(&self) -> &PLIC {
        self.plic.as_ref().unwrap()
    }

    pub fn get(&self, dev_id: &DevId) -> Option<&Arc<dyn BaseDeviceOps>> {
        self.devices.get(dev_id)
    }

    pub fn devices(&self) -> &BTreeMap<DevId, Arc<dyn BaseDeviceOps>> {
        &self.devices
    }

    pub fn enable_device_interrupts(&mut self) {
        for i in 0..HART_NUM * 2 {
            for dev in self.devices.values() {
                if let Some(irq) = dev.irq_no() {
                    self.plic().enable_irq(irq, i);
                    info!("Enable external interrupt:{irq}, context:{i}");
                }
            }
        }
        unsafe { enable_external_interrupt() }
    }

    pub fn handle_irq(&mut self) {
        unsafe { disable_interrupt() }

        log::info!("Handling interrupt");
        // First clain interrupt from PLIC
        if let Some(irq_number) = self.plic().claim_irq(self.irq_context()) {
            if let Some(dev) = self.irq_map.get(&irq_number) {
                info!(
                    "Handling interrupt from device: {:?}, irq: {}",
                    dev.name(),
                    irq_number
                );
                dev.handle_irq();
                // Complete interrupt when done
                self.plic().complete_irq(irq_number, self.irq_context());
                return;
            }
            warn!("Unknown interrupt: {}", irq_number);
            return;
        }
        warn!("No interrupt available");
    }

    // Calculate the interrupt context from current hart id
    fn irq_context(&self) -> usize {
        // TODO:
        1
    }
}
