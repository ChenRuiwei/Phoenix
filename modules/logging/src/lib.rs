#![no_std]
#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};

use crate_interface::call_interface;
use log::{Level, LevelFilter, Log, Metadata, Record};

pub static mut LOG_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn init() {
    static LOGGER: SimpleLogger = SimpleLogger;
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(match option_env!("LOG") {
        Some("error") => LevelFilter::Error,
        Some("warn") => LevelFilter::Warn,
        Some("info") => LevelFilter::Info,
        Some("debug") => LevelFilter::Debug,
        Some("trace") => LevelFilter::Trace,
        _ => LevelFilter::Error,
    });
    unsafe { LOG_INITIALIZED.store(true, Ordering::SeqCst) };
}

/// Add escape sequence to print with color in linux console
// #[macro_export]
// macro_rules! with_color {
//     ($args:ident, $color_code:ident) => {{
//         format_args!("\u{1B}[{}m{}\u{1B}[0m", $color_code as u8, $args)
//     }};
// }

#[macro_export]
macro_rules! with_color {
    ($color_code:expr, $($arg:tt)*) => {{
        format_args!("\u{1B}[{}m{}\u{1B}[m", $color_code as u8, format_args!($($arg)*))
    }};
}

#[crate_interface::def_interface]
pub trait LogIf: Send + Sync {
    fn print_log(record: &Record);
}

struct SimpleLogger;

impl Log for SimpleLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        call_interface!(LogIf::print_log(record));
    }
    fn flush(&self) {}
}

pub fn level_to_color_code(level: Level) -> u8 {
    match level {
        Level::Error => 31, // Red
        Level::Warn => 93,  // BrightYellow
        Level::Info => 36,  // Blue
        Level::Debug => 32, // Green
        Level::Trace => 90, // BrightBlack
    }
}

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum ColorCode {
    Black = 30,
    Red = 31,
    Green = 32,
    Yellow = 33,
    Blue = 34,
    Magenta = 35,
    Cyan = 36,
    White = 37,
    BrightBlack = 90,
    BrightRed = 91,
    BrightGreen = 92,
    BrightYellow = 93,
    BrightBlue = 94,
    BrightMagenta = 95,
    BrightCyan = 96,
    BrightWhite = 97,
}
