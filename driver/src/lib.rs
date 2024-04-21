#![no_std]
#![no_main]

extern crate alloc;

use alloc::sync::Arc;
use core::{
    fmt::{self, Write},
    task::Waker,
};

use qemu::virtio_blk::VirtIOBlkDev;
use spin::{Lazy, Once};
use sync::mutex::SpinNoIrqLock;

use self::sbi::console_putchar;

pub mod qemu;
pub mod sbi;

type Mutex<T> = SpinNoIrqLock<T>;

pub trait BlockDevice: Send + Sync {
    /// Read data form block to buffer
    fn read_blocks(&self, block_id: usize, buf: &mut [u8]);

    /// Write data from buffer to block
    fn write_blocks(&self, block_id: usize, buf: &[u8]);
}

pub trait CharDevice: Send + Sync {
    fn getchar(&self) -> u8;
    fn puts(&self, char: &[u8]);
    fn handle_irq(&self);
    fn register_waker(&self, _waker: Waker) {
        todo!()
    }
}

pub fn init() {
    init_block_device();
}

pub static BLOCK_DEVICE: Once<Arc<dyn BlockDevice>> = Once::new();

fn init_block_device() {
    BLOCK_DEVICE.call_once(|| Arc::new(VirtIOBlkDev::new()));
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
