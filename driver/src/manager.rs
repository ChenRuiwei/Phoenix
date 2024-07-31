//! Device manager
//!
//! Adapted from MankorOS

use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};

use arch::interrupts::{disable_interrupt, enable_external_interrupt};
use config::{board, mm::K_SEG_DTB_BEG};
use device_core::{DevId, Device};
use log::{info, warn};

use crate::{cpu::CPU, plic::PLIC, println};

/// The DeviceManager struct is responsible for managing the devices within the
/// system. It handles the initialization, probing, and interrupt management for
/// various devices.
pub struct DeviceManager {
    /// Optional PLIC (Platform-Level Interrupt Controller) to manage external
    /// interrupts.
    pub plic: Option<PLIC>,

    /// Vector containing CPU instances. The capacity is set to accommodate up
    /// to 8 CPUs.
    pub cpus: Vec<CPU>,

    /// A BTreeMap that maps device IDs (DevId) to device instances (Arc<dyn
    /// Device>). This map stores all the devices except for network devices
    /// which are managed separately by the `InterfaceWrapper` in the `net`
    /// module.
    pub devices: BTreeMap<DevId, Arc<dyn Device>>,

    /// A BTreeMap that maps interrupt numbers (irq_no) to device instances
    /// (Arc<dyn Device>). This map is used to quickly locate the device
    /// responsible for handling a specific interrupt.
    pub irq_map: BTreeMap<usize, Arc<dyn Device>>,
}

impl DeviceManager {
    /// Creates a new DeviceManager instance with default values.
    /// Initializes the PLIC to None, reserves space for 8 CPUs, and creates
    /// empty BTreeMaps for devices and irq_map.
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
        let device_tree =
            unsafe { fdt::Fdt::from_ptr(K_SEG_DTB_BEG as _).expect("Parse DTB failed") };
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

    /// Initializes all devices that have been discovered and added to the
    /// device manager.
    pub fn init_devices(&mut self) {
        for dev in self.devices.values() {
            dev.init();
        }
    }

    /// Retrieves a reference to the PLIC instance. Panics if PLIC is not
    /// initialized.
    fn plic(&self) -> &PLIC {
        self.plic.as_ref().unwrap()
    }

    pub fn get(&self, dev_id: &DevId) -> Option<&Arc<dyn Device>> {
        self.devices.get(dev_id)
    }

    pub fn devices(&self) -> &BTreeMap<DevId, Arc<dyn Device>> {
        &self.devices
    }

    pub fn enable_device_interrupts(&mut self) {
        for i in 0..board::harts() * 2 {
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
