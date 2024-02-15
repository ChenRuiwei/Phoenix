#![no_std]
#![no_main]

mod console;
mod sbi;

use core::arch::global_asm;
use core::panic::PanicInfo;

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
    println!("==================================================");
    println!("hello world");
    loop {
        wfi()
    }
}
