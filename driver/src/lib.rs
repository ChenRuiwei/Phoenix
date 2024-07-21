#![no_std]
#![no_main]
#![feature(trait_upcasting)]
#![feature(format_args_nl)]
#![feature(const_mut_refs)]
#![feature(const_slice_from_raw_parts_mut)]
#![feature(associated_type_defaults)]

extern crate alloc;

#[macro_use]
extern crate macro_utils;

use alloc::sync::Arc;
use core::fmt::{self, Write};

use async_utils::block_on;
use crate_interface::call_interface;
use device_core::{BlockDriverOps, CharDevice, DeviceMajor};
use early_print::EarlyStdout;
use manager::DeviceManager;
use memory::PageTable;
use spin::Once;
use sync::mutex::{SpinLock, SpinNoIrqLock};

use self::sbi::console_putchar;
use crate::serial::{Serial, UART0};

mod cpu;
mod manager;
pub mod net;
mod plic;
pub mod sbi;
pub mod serial;
pub mod virtio;

type Mutex<T> = SpinLock<T>;

pub fn init() {
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
        .filter(|(dev_id, _)| dev_id.major == DeviceMajor::Serial)
        .map(|(_, device)| {
            device
                .clone()
                .downcast_arc::<Serial>()
                .unwrap_or_else(|_| unreachable!())
        })
        .next()
        .unwrap();
    unsafe { UART0 = Some(serial) };
    // CHAR_DEVICE.call_once(|| manager.char_device[0].clone());
}

pub static BLOCK_DEVICE: Once<Arc<dyn BlockDriverOps>> = Once::new();

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
    fn kernel_page_table_mut() -> &'static mut PageTable;
}

pub(crate) fn kernel_page_table_mut() -> &'static mut PageTable {
    call_interface!(KernelPageTableIf::kernel_page_table_mut())
}

struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if let Some(serial) = unsafe { UART0.as_mut() } {
            block_on(async { serial.write(s.as_bytes()).await });
            Ok(())
        } else {
            EarlyStdout.write_str(s)
        }
    }
}

static PRINT_LOCK: SpinLock<()> = SpinLock::new(());
pub fn _print(args: fmt::Arguments) {
    let _guard = PRINT_LOCK.lock();
    Stdout.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::_print(format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
