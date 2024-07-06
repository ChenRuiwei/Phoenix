#![no_std]
#![no_main]
#![feature(trait_upcasting)]
#![feature(format_args_nl)]
#![feature(const_mut_refs)]
#![feature(const_slice_from_raw_parts_mut)]
#![feature(associated_type_defaults)]

#[macro_use]
extern crate alloc;

use alloc::{boxed::Box, collections::BTreeMap, sync::Arc};
use core::{
    fmt::{self, Write},
    task::Waker,
};

use async_trait::async_trait;
use async_utils::block_on;
use config::mm::{DTB_ADDR, VIRT_RAM_OFFSET};
use crate_interface::call_interface;
use device_core::{BaseDeviceOps, BlockDevice, CharDevice, DevId, DeviceMajor};
use manager::DeviceManager;
use memory::PageTable;
use qemu::virtio_blk::VirtIOBlkDev;
use spin::Once;
use sync::mutex::{SpinLock, SpinNoIrqLock};

use self::sbi::console_putchar;
use crate::serial::{Serial, UART0};

mod cpu;
mod manager;
pub mod net;
mod plic;
pub mod qemu;
pub mod sbi;
pub mod serial;

type Mutex<T> = SpinLock<T>;

pub fn init(dtb_addr: usize) {
    // init_block_device();
    init_device_manager();
    let manager = get_device_manager_mut();
    manager.probe();
    manager.init_devices();
    log::info!("Device initialization complete");
    manager.enable_device_interrupts();
    log::info!("External interrupts enabled");
    let serial = manager
        .devices()
        .iter()
        .filter(|(dev_id, device)| dev_id.major == DeviceMajor::Serial)
        .map(|(_, device)| {
            device
                .clone()
                .downcast_arc::<Serial>()
                .unwrap_or_else(|_| unreachable!())
        })
        .next()
        .unwrap();
    unsafe { *UART0.lock() = Some(serial) };
    // CHAR_DEVICE.call_once(|| manager.char_device[0].clone());
}

pub static BLOCK_DEVICE: Once<Arc<dyn BlockDevice>> = Once::new();

// fn init_block_device() {
//     BLOCK_DEVICE.call_once(|| VirtIOBlkDev::new());
// }

static mut DEVICE_MANAGER: Option<DeviceManager> = None;

pub fn get_device_manager() -> &'static DeviceManager {
    unsafe { DEVICE_MANAGER.as_ref().unwrap() }
}

pub fn get_device_manager_mut() -> &'static mut DeviceManager {
    unsafe { DEVICE_MANAGER.as_mut().unwrap() }
}

pub fn init_device_manager() {
    unsafe {
        DEVICE_MANAGER = Some(DeviceManager::new());
    }
}

#[crate_interface::def_interface]
pub trait KernelPageTableIf: Send + Sync {
    fn kernel_page_table() -> &'static mut PageTable;
}

pub(crate) fn kernel_page_table() -> &'static mut PageTable {
    call_interface!(KernelPageTableIf::kernel_page_table())
}

struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if let Some(serial) = unsafe { UART0.lock().as_mut() } {
            block_on(async { serial.write(s.as_bytes()).await });
            return Ok(());
        }
        for s in s.as_bytes() {
            console_putchar(*s as usize);
        }
        Ok(())
    }
}

pub fn print(args: fmt::Arguments<'_>) {
    Stdout.write_fmt(args).unwrap();
}

/// print string macro
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::print(format_args!($($arg)*));
    }};
}

/// println string macro
#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {{
        $crate::print(format_args_nl!($($arg)*));
    }};
}

pub fn shutdown() -> ! {
    sbi::shutdown()
}

pub fn set_timer(timer: usize) {
    sbi::set_timer(timer)
}
