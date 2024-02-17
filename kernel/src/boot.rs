use core::arch::asm;

use crate::{println, rust_main};
use config::mm::VIRT_RAM_OFFSET;

const BOOT_MSG: &str = r#"
===============================================================
  __________  ________ _______  __  _____________   ___  ______
 / ___/ __/ |/ / __/ // /  _/ |/ / / __/_  __/ _ | / _ \/_  __/
/ (_ / _//    /\ \/ _  // //    / _\ \  / / / __ |/ , _/ / /
\___/___/_/|_/___/_//_/___/_/|_/ /___/ /_/ /_/ |_/_/|_| /_/
===============================================================
"#;

pub fn print_boot_message() {
    println!("{}", BOOT_MSG);
}

#[no_mangle]
unsafe fn fake_main() {
    asm!(
        "add sp, sp, {0}",
        "la t0, {_main}",
        "add t0, t0, {0}",
        "jr t0",
        in(reg) VIRT_RAM_OFFSET,
        _main = sym rust_main
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
