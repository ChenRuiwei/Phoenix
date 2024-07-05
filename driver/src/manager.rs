//! Device manager

use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};

use arch::interrupts::{disable_interrupt, enable_external_interrupt};
use config::processor::HART_NUM;
use device_core::{BaseDeviceOps, DevId};
use log::{info, warn};

use super::{plic, CharDevice};
use crate::{
    cpu::{self, CPU},
    plic::PLIC,
    qemu::virtio_net::{self, NetDevice},
    serial,
};
pub enum DeviceEnum {
    /// Network card device.
    Net(NetDevice),
    // Block storage device.
    // Block(AxBlockDevice),
    // Display(AxDisplayDevice),
}
pub trait DriverProbe {
    fn probe_mmio(_mmio_base: usize, _mmio_size: usize) -> Option<DeviceEnum> {
        None
    }
}

pub struct DeviceManager {
    plic: Option<PLIC>,
    cpus: Vec<CPU>,
    devices: BTreeMap<DevId, Arc<dyn BaseDeviceOps>>,
    /// irq_no -> device.
    irq_map: BTreeMap<usize, Arc<dyn BaseDeviceOps>>,
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
        // Probe PLIC
        self.plic = Some(plic::probe());
        let char_device = Arc::new(serial::probe().unwrap());
        self.devices
            .insert(char_device.dev_id(), char_device.clone());
        let net_device = virtio_net::probe();

        self.cpus.extend(cpu::probe());

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
