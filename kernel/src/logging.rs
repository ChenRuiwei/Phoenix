use log::{debug, error, info, trace, warn, Level, LevelFilter, Log, Metadata, Record};

use crate::println;

struct SimpleLogger;

impl Log for SimpleLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let color = match record.level() {
            Level::Error => 31, // Red
            Level::Warn => 93,  // BrightYellow
            Level::Info => 34,  // Blue
            Level::Debug => 32, // Green
            Level::Trace => 90, // BrightBlack
        };
        println!(
            "\u{1B}[{}m[{:>5}] {}\u{1B}[0m",
            color,
            record.level(),
            record.args(),
        );
    }
    fn flush(&self) {}
}

pub fn init() {
    static LOGGER: SimpleLogger = SimpleLogger;
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(match option_env!("LOG") {
        Some("error") => LevelFilter::Error,
        Some("warn") => LevelFilter::Warn,
        Some("info") => LevelFilter::Info,
        Some("debug") => LevelFilter::Debug,
        Some("trace") => LevelFilter::Trace,
        _ => LevelFilter::Off,
    });
}

#[allow(dead_code)]
pub fn show_examples() {
    extern "C" {
        fn _stext();
        fn _etext();
        fn _srodata();
        fn _erodata();
        fn _sdata();
        fn _edata();
        fn _sstack();
        fn _estack();
        fn _sbss();
        fn _ebss();
    }
    error!(
        "stext: {:#x}, etext: {:#x}",
        _stext as usize, _etext as usize
    );
    warn!(
        "srodata: {:#x}, erodata: {:#x}",
        _srodata as usize, _erodata as usize
    );
    info!(
        "sdata: {:#x}, edata: {:#x}",
        _sdata as usize, _edata as usize
    );
    debug!(
        "sstack: {:#x}, estack: {:#x}",
        _sstack as usize, _estack as usize
    );
    trace!("sbss: {:#x}, ebss: {:#x}", _sbss as usize, _ebss as usize);
}
