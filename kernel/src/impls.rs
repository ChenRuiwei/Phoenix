//! Impls of traits defined in other crates.

use alloc::fmt;

use logging::LogIf;

use crate::processor::hart::local_hart;

/// Print msg with color
pub fn print_in_color(args: fmt::Arguments, color_code: u8) {
    driver::print(with_color!(args, color_code));
}

struct LogIfImpl;

#[crate_interface::impl_interface]
impl LogIf for LogIfImpl {
    fn print_log(record: &log::Record) {
        print_in_color(
            format_args!(
                "[{:>5}][{}:{}][{},-,-] {}\n",
                record.level(),
                record.file().unwrap(),
                record.line().unwrap(),
                local_hart().hart_id(),
                record.args()
            ),
            logging::level_to_color_code(record.level()),
        );
    }
}

