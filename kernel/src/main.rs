#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

mod boot;
mod console;
mod logging;
mod mm;
mod sbi;

use core::{arch::asm, panic::PanicInfo};

// Wait for interrupt, allows the CPU to go into a power saving mode
pub fn wfi() {
    unsafe { asm!("wfi") }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        wfi()
    }
}

#[no_mangle]
pub fn rust_main(hart_id: usize) {
    boot::clear_bss();
    boot::print_boot_message();
    logging::init();
    logging::show_examples();

    mm::heap_allocator::init_heap();
    mm::heap_allocator::heap_test();

    sbi::shutdown(true);
}
