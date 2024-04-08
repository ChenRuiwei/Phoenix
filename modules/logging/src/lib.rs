#![no_std]
#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};

extern crate crate_interface;

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
#[macro_export]
macro_rules! with_color {
    ($args:ident, $color_code:ident) => {{
        format_args!("\u{1B}[{}m{}\u{1B}[0m", $color_code as u8, $args)
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
