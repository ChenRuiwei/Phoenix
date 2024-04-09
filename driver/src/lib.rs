#![no_std]
#![no_main]

extern crate alloc;
use alloc::sync::Arc;
use core::{
    fmt::{self, Write},
    task::Waker,
};

use sync::mutex::SpinNoIrqLock;

use self::sbi::console_putchar;

pub mod sbi;

type Mutex<T> = SpinNoIrqLock<T>;

static PRINT_MUTEX: Mutex<()> = Mutex::new(());

// Block Device
pub trait BlockDevice: Send + Sync {
    /// Read data form block to buffer
    fn read_block(&self, block_id: usize, buf: &mut [u8]);
    /// Write data from buffer to block
    fn write_block(&self, block_id: usize, buf: &[u8]);
}

// Character Device
pub trait CharDevice: Send + Sync {
    fn getchar(&self) -> u8;
    fn puts(&self, char: &[u8]);
    fn handle_irq(&self);
    fn register_waker(&self, _waker: Waker) {
        todo!()
    }
}

// Net Device
pub trait NetDevice: smoltcp::phy::Device {}

pub static BLOCK_DEVICE: Mutex<Option<Arc<dyn BlockDevice>>> = Mutex::new(None);

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
    let _lock = PRINT_MUTEX.lock();
    Stdout.write_fmt(args).unwrap();
}

/// print string macro
#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::print(format_args!($fmt $(, $($arg)+)?));
    }
}

/// println string macro
#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?));
    }
}

pub fn shutdown() -> ! {
    sbi::shutdown()
}

pub fn set_timer(timer: usize) {
    sbi::set_timer(timer)
}
