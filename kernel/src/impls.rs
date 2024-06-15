//! Impls of traits defined in other crates.

use alloc::{fmt, string::ToString};

use log::Level;
use logging::{level_to_color_code, ColorCode, LogIf};

use crate::processor::hart::{current_task_ref, local_hart};

/// Print msg with color
pub fn print_in_color(args: fmt::Arguments, color_code: u8) {
    driver::print(with_color!(color_code, "{}", args));
}

struct LogIfImpl;

#[crate_interface::impl_interface]
impl LogIf for LogIfImpl {
    fn print_log(record: &log::Record) {
        let level = record.level();
        let level_color = match level {
            Level::Error => ColorCode::BrightRed,
            Level::Warn => ColorCode::BrightYellow,
            Level::Info => ColorCode::BrightGreen,
            Level::Debug => ColorCode::BrightCyan,
            Level::Trace => ColorCode::BrightBlack,
        };
        let args_color = match level {
            Level::Error => ColorCode::Red,
            Level::Warn => ColorCode::Yellow,
            Level::Info => ColorCode::Green,
            Level::Debug => ColorCode::Cyan,
            Level::Trace => ColorCode::BrightBlack,
        };
        let line = record.line().unwrap_or(0);
        let target = record.file().unwrap_or("");
        let args = record.args();
        let hid = local_hart().hart_id();
        let pid = if local_hart().has_task() {
            current_task_ref().pid().to_string()
        } else {
            "-".to_string()
        };
        let tid = if local_hart().has_task() {
            current_task_ref().tid().to_string()
        } else {
            "-".to_string()
        };
        driver::print(with_color!(
            ColorCode::White,
            "{}{}{} {} \r\n",
            with_color!(level_color, "[{:>5}]", level),
            with_color!(ColorCode::White, "[{:>35}:{:<4}]", target, line),
            with_color!(ColorCode::Blue, "[H{},P{},T{}]", hid, pid, tid),
            with_color!(args_color, "{}", args),
        ));
    }
}
