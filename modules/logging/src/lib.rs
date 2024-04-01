#![no_std]
#![no_main]

use core::{default, marker::PhantomData};

use log::{Level, LevelFilter, Log, Metadata, Record};

pub fn init<P: LOGGING>(logger: &'static SimpleLogger<P>) {
    log::set_logger(logger).unwrap();
    log::set_max_level(match option_env!("LOG") {
        Some("error") => LevelFilter::Error,
        Some("warn") => LevelFilter::Warn,
        Some("info") => LevelFilter::Info,
        Some("debug") => LevelFilter::Debug,
        Some("trace") => LevelFilter::Trace,
        _ => LevelFilter::Error,
    });
}

/// Add escape sequence to print with color in Linux console
#[macro_export]
macro_rules! with_color {
    ($args:ident, $color_code:ident) => {{
        format_args!("\u{1B}[{}m{}\u{1B}[0m", $color_code as u8, $args)
    }};
}

pub trait LOGGING: Send + Sync {
    fn print_log(record: &Record);
}

pub struct SimpleLogger<P: LOGGING> {
    _phantom: PhantomData<P>,
}

impl<P: LOGGING> SimpleLogger<P> {
    pub const fn new() -> Self {
        SimpleLogger {
            _phantom: PhantomData,
        }
    }
}

impl<P: LOGGING> Log for SimpleLogger<P> {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        P::print_log(record);
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
