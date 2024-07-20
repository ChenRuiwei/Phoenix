//! Device manager

use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};

use arch::interrupts::{disable_interrupt, enable_external_interrupt};
use config::{
    mm::{DTB_ADDR, VIRT_RAM_OFFSET},
    processor::HART_NUM,
};
use device_core::{BaseDriverOps, DevId};
use log::{info, warn};

use crate::{cpu::CPU, plic::PLIC, println};

pub struct DeviceManager {
    pub plic: Option<PLIC>,
    pub cpus: Vec<CPU>,
    /// net device is excluded from `device`. It is owned by `InterfaceWrapper`
    /// in `net` module
    pub devices: BTreeMap<DevId, Arc<dyn BaseDriverOps>>,
    /// irq_no -> device.
    pub irq_map: BTreeMap<usize, Arc<dyn BaseDriverOps>>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            plic: None,
            cpus: Vec::with_capacity(8),
            devices: BTreeMap::new(),
            irq_map: BTreeMap::new(),
        }
    }

    /// mmio memory region map finished in this function
    pub fn probe(&mut self) {
        let device_tree = unsafe {
            fdt::Fdt::from_ptr((DTB_ADDR + VIRT_RAM_OFFSET) as _).expect("Parse DTB failed")
        };
        if let Some(bootargs) = device_tree.chosen().bootargs() {
            println!("Bootargs: {:?}", bootargs);
        }
        println!("Device: {}", device_tree.root().model());

        // Probe PLIC
        self.probe_plic(&device_tree);

        // Probe serial console
        self.probe_char_device(&device_tree);

        self.probe_cpu(&device_tree);

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

    fn plic(&self) -> &PLIC {
        self.plic.as_ref().unwrap()
    }

    pub fn get(&self, dev_id: &DevId) -> Option<&Arc<dyn BaseDriverOps>> {
        self.devices.get(dev_id)
    }

    pub fn devices(&self) -> &BTreeMap<DevId, Arc<dyn BaseDriverOps>> {
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

        log::trace!("Handling interrupt");
        // First clain interrupt from PLIC
        if let Some(irq_number) = self.plic().claim_irq(self.irq_context()) {
            if let Some(dev) = self.irq_map.get(&irq_number) {
                log::trace!(
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
        } else {
            warn!("No interrupt available");
        }
    }

    // Calculate the interrupt context from current hart id
    fn irq_context(&self) -> usize {
        // TODO:
        1
    }
}
