pub mod uart8250;

use alloc::{boxed::Box, collections::VecDeque, string::ToString};
use core::{
    cell::UnsafeCell,
    fmt::{Debug, Write},
    future::Future,
    pin::Pin,
    task::{Poll, Waker},
};

use async_trait::async_trait;
use config::mm::{DTB_ADDR, VIRT_RAM_OFFSET};
use device_core::{DevId, Device, DeviceMajor, DeviceMeta, DeviceType};
use log::{info, warn};
use ringbuffer::RingBuffer;
use sync::mutex::SpinNoIrqLock;

use super::CharDevice;
use crate::{println, serial::uart8250::Uart};

trait UartDriver: Send + Sync {
    fn init(&mut self);
    fn putchar(&mut self, byte: u8);
    fn getchar(&mut self) -> Option<u8>;
}

pub struct Serial {
    meta: DeviceMeta,
    inner: UnsafeCell<Box<dyn UartDriver>>,
    buffer: SpinNoIrqLock<ringbuffer::ConstGenericRingBuffer<u8, 512>>, // Hard-coded buffer size
    waiting: SpinNoIrqLock<VecDeque<Waker>>,
}

unsafe impl Send for Serial {}
unsafe impl Sync for Serial {}

impl Serial {
    fn new(mmio_base: usize, mmio_size: usize, irq_no: usize, driver: Box<dyn UartDriver>) -> Self {
        let meta = DeviceMeta {
            dev_id: DevId {
                major: DeviceMajor::Serial,
                minor: 0,
            },
            name: "serial".to_string(),
            mmio_base,
            mmio_size,
            irq_no: Some(irq_no),
            dtype: DeviceType::Char,
        };

        Self {
            meta,
            inner: UnsafeCell::new(driver),
            buffer: SpinNoIrqLock::new(ringbuffer::ConstGenericRingBuffer::new()),
            waiting: SpinNoIrqLock::new(VecDeque::new()),
        }
    }
}

impl Debug for Serial {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Serial")
    }
}

impl Device for Serial {
    fn meta(&self) -> &device_core::DeviceMeta {
        &self.meta
    }

    fn handle_irq(&self) {
        todo!()
    }

    fn init(&self) {
        unsafe { &mut *self.inner.get() }.as_mut().init()
    }
}

#[async_trait]
impl CharDevice for Serial {
    async fn getchar(&self) -> u8 {
        0
        // (unsafe { *self.inner.get() }).getchar().unwrap()
    }

    async fn puts(&self, char: &[u8]) {
        for &c in char {
            // (unsafe { *self.inner.get() }).putchar(c)
        }
    }

    fn poll_in(&self) -> bool {
        todo!()
    }

    fn poll_out(&self) -> bool {
        todo!()
    }

    fn handle_irq(&self) {
        todo!()
    }
}

pub fn probe() -> Option<Serial> {
    let device_tree =
        unsafe { fdt::Fdt::from_ptr((DTB_ADDR + VIRT_RAM_OFFSET) as _).expect("Parse DTB failed") };
    let chosen = device_tree.chosen();
    if let Some(bootargs) = chosen.bootargs() {
        println!("Bootargs: {:?}", bootargs);
    }

    println!("Device: {}", device_tree.root().model());

    // Serial
    let mut stdout = chosen.stdout();
    if stdout.is_none() {
        println!("Non-standard stdout device, trying to workaround");
        let chosen = device_tree.find_node("/chosen").expect("No chosen node");
        let stdout_path = chosen
            .properties()
            .find(|n| n.name == "stdout-path")
            .and_then(|n| {
                let bytes = unsafe {
                    core::slice::from_raw_parts_mut((n.value.as_ptr()) as *mut u8, n.value.len())
                };
                let mut len = 0;
                for byte in bytes.iter() {
                    if *byte == b':' {
                        return core::str::from_utf8(&n.value[..len]).ok();
                    }
                    len += 1;
                }
                core::str::from_utf8(&n.value[..n.value.len() - 1]).ok()
            })
            .unwrap();
        println!("Searching stdout: {}", stdout_path);
        stdout = device_tree.find_node(stdout_path);
    }
    if stdout.is_none() {
        println!("Unable to parse /chosen, choosing first serial device");
        stdout = device_tree.find_compatible(&[
            "ns16550a",
            "snps,dw-apb-uart", // C910, VF2
            "sifive,uart0",     // sifive_u QEMU (FU540)
        ])
    }
    let stdout = stdout.expect("Still unable to get stdout device");
    println!("Stdout: {}", stdout.name);

    Some(probe_serial_console(&stdout))
}

/// This guarantees to return a Serial device
/// The device is not initialized yet
fn probe_serial_console(stdout: &fdt::node::FdtNode) -> Serial {
    let reg = stdout.reg().unwrap().next().unwrap();
    let base_paddr = reg.starting_address as usize;
    let size = reg.size.unwrap();
    let base_vaddr = base_paddr + VIRT_RAM_OFFSET;
    let irq_number = stdout.property("interrupts").unwrap().as_usize().unwrap();
    info!("IRQ number: {}", irq_number);

    let first_compatible = stdout.compatible().unwrap().first();
    match first_compatible {
        "ns16550a" | "snps,dw-apb-uart" => {
            // VisionFive 2 (FU740)
            // virt QEMU

            // Parse clock frequency
            let freq_raw = stdout
                .property("clock-frequency")
                .expect("No clock-frequency property of stdout serial device")
                .as_usize()
                .expect("Parse clock-frequency to usize failed");
            let mut reg_io_width = 1;
            if let Some(reg_io_width_raw) = stdout.property("reg-io-width") {
                reg_io_width = reg_io_width_raw
                    .as_usize()
                    .expect("Parse reg-io-width to usize failed");
            }
            let mut reg_shift = 0;
            if let Some(reg_shift_raw) = stdout.property("reg-shift") {
                reg_shift = reg_shift_raw
                    .as_usize()
                    .expect("Parse reg-shift to usize failed");
            }
            log::info!("uart: base_paddr:{base_paddr:#x}, size:{size:#x}, reg_io_width:{reg_io_width}, reg_shift:{reg_shift}");

            let uart = unsafe {
                Uart::new(
                    base_vaddr,
                    freq_raw,
                    115200,
                    reg_io_width,
                    reg_shift,
                    first_compatible == "snps,dw-apb-uart",
                )
            };
            Serial::new(base_paddr, size, irq_number, Box::new(uart))
        }
        "sifive,uart0" => {
            todo!()
            // sifive_u QEMU (FU540)
            // let uart = sifive::SifiveUart::new(
            //     base_vaddr,
            //     500 * 1000 * 1000, // 500 MHz hard coded for now
            // );
            // Serial::new(base_paddr, size, irq_number, Box::new(uart))
        }
        _ => panic!("Unsupported serial console"),
    }
}
