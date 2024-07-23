#![no_std]
#![no_main]

use core::{fmt, fmt::Write};

pub struct SbiStdout;

impl fmt::Write for SbiStdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for s in s.as_bytes() {
            sbi_rt::legacy::console_putchar(*s as usize);
        }
        Ok(())
    }
}

pub fn _sbi_print(args: fmt::Arguments<'_>) {
    SbiStdout.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! sbi_print {
    ($($arg:tt)*) => {{
        $crate::_sbi_print(format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! sbi_println {
    () => ($crate::sbi_print!("\n"));
    ($($arg:tt)*) => ($crate::sbi_print!("{}\n", format_args!($($arg)*)));
}
