#![no_std]
#![no_main]

use core::mem::size_of;

use sbi_print::sbi_println;

pub fn backtrace() {
    extern "C" {
        fn _stext();
        fn _etext();
    }
    unsafe {
        let mut current_pc = arch::register::ra();
        let mut current_fp = arch::register::fp();

        while current_pc >= _stext as usize && current_pc <= _etext as usize && current_fp != 0 {
            sbi_println!("{:#018x}", current_pc - size_of::<usize>());
            current_fp = *(current_fp as *const usize).offset(-2);
            current_pc = *(current_fp as *const usize).offset(-1);
        }
    }
}
