#![no_std]
#![no_main]

extern crate alloc;

use alloc::sync::Arc;
use core::{
    fmt::{self, Write},
    task::Waker,
};

use device_core::{BlockDevice, CharDevice};
use qemu::{uart::UartDevice, virtio_blk::VirtIOBlkDev};
use spin::Once;
use sync::mutex::SpinNoIrqLock;

use self::sbi::console_putchar;

mod buffer_cache;
pub mod qemu;
pub mod sbi;

type Mutex<T> = SpinNoIrqLock<T>;

pub fn init() {
    init_block_device();
    init_char_device();
}

pub static CHAR_DEVICE: Once<Arc<dyn CharDevice>> = Once::new();
pub static BLOCK_DEVICE: Once<Arc<dyn BlockDevice>> = Once::new();

fn init_block_device() {
    BLOCK_DEVICE.call_once(|| VirtIOBlkDev::new());
}

fn init_char_device() {
    CHAR_DEVICE.call_once(|| Arc::new(UartDevice::new()));
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

pub fn getchar() -> u8 {
    let char_device = CHAR_DEVICE.get().unwrap();
    char_device.getchar()
}

pub fn print(args: fmt::Arguments<'_>) {
    static PRINT_MUTEX: Mutex<()> = Mutex::new(());
    let _lock = PRINT_MUTEX.lock();
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
