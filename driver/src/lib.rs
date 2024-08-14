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

use ::net::init_network;
use async_utils::block_on;
use config::{board::clock_freq, mm::K_SEG_DTB_BEG};
use crate_interface::call_interface;
use device_core::{BlockDevice, CharDevice, DeviceMajor, DeviceType};
use manager::DeviceManager;
use memory::PageTable;
use sbi_print::SbiStdout;
use spin::Once;
use sync::mutex::{SpinLock, SpinNoIrqLock};
use virtio_drivers::transport;

use crate::{
    net::loopback::LoopbackDev,
    serial::{Serial, UART0},
};

mod blk;
mod cpu;
mod manager;
pub mod net;
mod plic;
pub mod serial;
pub mod virtio;

type Mutex<T> = SpinLock<T>;

pub fn init() {
    let device_tree = unsafe { fdt::Fdt::from_ptr(K_SEG_DTB_BEG as _).expect("Parse DTB failed") };
    config::board::set_clock_freq(device_tree.cpus().next().unwrap().timebase_frequency());
    log::info!("clock freq set to {} Hz", clock_freq());

    init_device_manager();
    let manager = get_device_manager_mut();
    manager.probe();
    manager.map_devices();
    manager.init_devices();

    log::info!("Device initialization complete");
    manager.enable_device_interrupts();
    log::info!("External interrupts enabled");
    let serial = manager
        .find_devices_by_major(DeviceMajor::Serial)
        .into_iter()
        .map(|device| device.as_char().unwrap())
        .next()
        .unwrap();
    UART0.call_once(|| serial.clone());

    let blk = manager
        .find_devices_by_major(DeviceMajor::Block)
        .into_iter()
        .map(|device| device.as_blk().unwrap())
        .next()
        .unwrap();
    BLOCK_DEVICE.call_once(|| blk.clone());

    log::info!("[init_net] can't find qemu virtio-net. use LoopbackDev to test");
    init_network(LoopbackDev::new(), true);
}

pub static BLOCK_DEVICE: Once<Arc<dyn BlockDevice>> = Once::new();

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
        if let Some(serial) = unsafe { UART0.get() } {
            block_on(async { serial.write(s.as_bytes()).await });
            Ok(())
        } else {
            SbiStdout.write_str(s)
        }
    }
}

static PRINT_LOCK: SpinNoIrqLock<()> = SpinNoIrqLock::new(());
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

macro_rules! wait_for {
    ($cond:expr) => {{
        let mut timeout = 10000000;
        while !$cond && timeout > 0 {
            core::hint::spin_loop();
            timeout -= 1;
        }
    }};
}
pub(crate) use wait_for;
