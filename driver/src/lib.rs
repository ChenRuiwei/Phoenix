#![no_std]
#![no_main]

extern crate alloc;
use alloc::{boxed::Box, sync::Arc};
use core::{
    fmt::{self, Write},
    task::Waker,
};

use arch::interrupts;
use memory::PageTable;
use sync::{cell::SyncUnsafeCell, mutex::SpinNoIrqLock};

use self::{
    plic::{initplic, PLIC},
    qemu::virtio_blk::VirtIOBlock,
    sbi::console_putchar,
};

pub mod plic;
pub mod qemu;
pub mod sbi;

type Mutex<T> = SpinNoIrqLock<T>;

static PRINT_MUTEX: Mutex<()> = Mutex::new(());

pub fn intr_handler(hart_id: usize) {
    let mut plic = PLIC::new(0xffff_ffc0_0c00_0000);
    let context_id = hart_id * 2;
    let intr = plic.claim(context_id);
    use qemu::IntrSource;

    if intr != 0 {
        match intr.into() {
            IntrSource::UART0 => {
                // uart
                log::info!("receive uart0 intr");
                CHAR_DEVICE
                    .get_unchecked_mut()
                    .as_ref()
                    .unwrap()
                    .handle_irq();
            }
            IntrSource::VIRTIO0 => {
                // sdcard
                log::info!("receive virtio0 intr");
            }
            _ => {
                panic!("unexpected interrupt {}", intr);
            }
        }
        plic.complete(context_id, intr);
    } else {
        log::info!("didn't claim any intr");
    }
}

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
pub static CHAR_DEVICE: SyncUnsafeCell<Option<Box<dyn CharDevice>>> = SyncUnsafeCell::new(None);
pub static mut KERNEL_PAGE_TABLE: Option<Arc<SyncUnsafeCell<PageTable>>> = None;

struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let char_device = CHAR_DEVICE.get_unchecked_mut();
        if let Some(cd) = char_device.as_ref() {
            cd.puts(s.as_bytes());
        } else {
            for s in s.as_bytes() {
                console_putchar(*s as usize);
            }
        }
        Ok(())
    }
}

pub fn getchar() -> u8 {
    let char_device = CHAR_DEVICE.get_unchecked_mut();
    if let Some(cd) = char_device.as_ref() {
        cd.clone().getchar()
    } else {
        0xff
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
        driver::print(format_args!($fmt $(, $($arg)+)?));
    }
}

/// println string macro
#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        driver::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?));
    }
}

pub fn shutdown() -> ! {
    sbi::shutdown()
}

pub fn set_timer(timer: usize) {
    sbi::set_timer(timer)
}
