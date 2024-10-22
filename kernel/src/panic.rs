//! The panic handler
use core::{
    panic::PanicInfo,
    sync::atomic::{AtomicUsize, Ordering},
};

use arch::interrupts::disable_interrupt;
use backtrace::backtrace;
use logging::LOG_INITIALIZED;
use sbi_print::sbi_println;
use sbi_rt::legacy::shutdown;

use crate::processor::hart::local_hart;

static PANIC_CNT: AtomicUsize = AtomicUsize::new(0);

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe { disable_interrupt() };

    sbi_println!("early panic now!!!");
    if PANIC_CNT.fetch_add(1, Ordering::Relaxed) > 0 {
        unsafe { LOG_INITIALIZED.store(false, Ordering::Relaxed) }
        if let Some(location) = info.location() {
            sbi_println!(
                "Hart {} panic at {}:{}, msg: {}",
                local_hart().hart_id(),
                location.file(),
                location.line(),
                info.message().unwrap()
            );
        } else if let Some(msg) = info.message() {
            sbi_println!("Panicked: {}", msg);
        } else {
            sbi_println!("Unknown panic: {:?}", info);
        }
        backtrace();
        shutdown()
    }

    println!("panic now!!!");

    // NOTE: message below is mostly printed in log, if these messages can not be
    // printed, it means some of the message will cause panic again, check
    // `LogIf::print_log`.
    let logging_initialized = unsafe { logging::LOG_INITIALIZED.load(Ordering::SeqCst) };
    if let Some(location) = info.location() {
        if logging_initialized {
            log::error!(
                "Hart {} panic at {}:{}, msg: {}",
                local_hart().hart_id(),
                location.file(),
                location.line(),
                info.message().unwrap()
            );
        } else {
            println!(
                "Hart {} panic at {}:{}, msg: {}",
                local_hart().hart_id(),
                location.file(),
                location.line(),
                info.message().unwrap()
            );
        }
    } else if let Some(msg) = info.message() {
        if logging_initialized {
            log::error!("Panicked: {}", msg);
        } else {
            println!("Panicked: {}", msg);
        }
    } else if logging_initialized {
        log::error!("Unknown panic: {:?}", info);
    } else {
        println!("Unknown panic: {:?}", info);
    }

    log::error!("=============== BEGIN BACKTRACE ================");
    backtrace();
    log::error!("=============== END BACKTRACE ================");

    shutdown()
}
