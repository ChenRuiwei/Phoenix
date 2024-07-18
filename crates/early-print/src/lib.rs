#![no_std]
#![no_main]

use core::{fmt, fmt::Write};

struct EarlyStdout;

impl fmt::Write for EarlyStdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for s in s.as_bytes() {
            sbi_rt::legacy::console_putchar(*s as usize);
        }
        Ok(())
    }
}

pub fn early_print(args: fmt::Arguments<'_>) {
    EarlyStdout.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! early_print {
    ($($arg:tt)*) => {{
        $crate::early_print(format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! early_println {
    () => ($crate::early_print!("\n"));
    ($($arg:tt)*) => ($crate::early_print!("{}\n", format_args!($($arg)*)));
}
