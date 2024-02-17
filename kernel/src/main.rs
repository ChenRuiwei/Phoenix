#![no_std]
#![no_main]

mod boot;
mod console;
mod logging;
mod sbi;

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

use log::{debug, error, info, trace, warn};

global_asm!(include_str!("entry.S"));

// Wait for interrupt, allows the CPU to go into a power saving mode
pub fn wfi() {
    unsafe { core::arch::asm!("wfi") }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        wfi()
    }
}

#[no_mangle]
pub fn rust_main() -> ! {
    boot::clear_bss();
    boot::print_boot_message();
    logging::init();
    logging::show_examples();

    loop {
        wfi()
    }
}
