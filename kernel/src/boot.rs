use core::arch::asm;

use crate::rust_main;
use config::mm::VIRT_RAM_OFFSET;

#[no_mangle]
unsafe fn fake_main() {
    asm!(
        "add sp, sp, {0}",
        "la t0, {_rust_main}",
        "add t0, t0, {0}",
        "jr t0",
        in(reg) VIRT_RAM_OFFSET,
        _rust_main = sym rust_main,
    );
}

/// Clear BSS segment at start up
pub fn clear_bss() {
    extern "C" {
        static _sbss: usize;
        static _ebss: usize;
    }
    unsafe {
        (_sbss.._ebss).for_each(|a| (a as *mut u8).write_volatile(0));
    }
}
