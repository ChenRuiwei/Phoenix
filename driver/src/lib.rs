#![no_std]
#![no_main]
#![feature(trait_upcasting)]
#![feature(format_args_nl)]

extern crate alloc;

use alloc::{boxed::Box, collections::BTreeMap, sync::Arc};
use core::{
    fmt::{self, Write},
    task::Waker,
};

use async_trait::async_trait;
use device_core::{CharDevice, DevId, Device};
use manager::DeviceManager;
use qemu::virtio_blk::VirtIOBlkDev;
use spin::Once;
use sync::mutex::{SpinLock, SpinNoIrqLock};

use self::sbi::console_putchar;

mod cpu;
mod manager;
mod plic;
pub mod qemu;
pub mod sbi;
pub mod serial;

type Mutex<T> = SpinLock<T>;

pub trait BlockDevice: Send + Sync {
    fn size(&self) -> u64;

    fn block_size(&self) -> usize;

    /// Read data form block to buffer
    fn read_blocks(&self, block_id: usize, buf: &mut [u8]);

    /// Write data from buffer to block
    fn write_blocks(&self, block_id: usize, buf: &[u8]);
}

pub fn init(dtb_addr: usize) {
    init_block_device();
    init_device_manager();
    let manager = get_device_manager_mut();
    manager.probe();
    manager.init_devices();
    log::info!("Device initialization complete");
    manager.enable_device_interrupts();
    log::info!("External interrupts enabled");
    // CHAR_DEVICE.call_once(|| manager.char_device[0].clone());
}

pub static DEVICES: Once<BTreeMap<DevId, Arc<dyn Device>>> = Once::new();

pub static BLOCK_DEVICE: Once<Arc<dyn BlockDevice>> = Once::new();

fn init_block_device() {
    BLOCK_DEVICE.call_once(|| Arc::new(VirtIOBlkDev::new()));
}

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

struct Stdout;

impl Write for Stdout {
    // TODO: char device support
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for s in s.as_bytes() {
            console_putchar(*s as usize);
        }
        Ok(())
    }
}

pub fn print(args: fmt::Arguments<'_>) {
    // static PRINT_MUTEX: Mutex<()> = Mutex::new(());
    // let _lock = PRINT_MUTEX.lock();
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
