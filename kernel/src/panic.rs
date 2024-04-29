//! The panic handler
use core::{mem::size_of, panic::PanicInfo, sync::atomic::Ordering};

use arch::interrupts::disable_interrupt;
use driver::sbi::shutdown;

use crate::processor::hart::local_hart;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe { disable_interrupt() };

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

    backtrace();
    shutdown()
}

fn backtrace() {
    extern "C" {
        fn _stext();
        fn _etext();
    }
    unsafe {
        let mut current_pc = arch::register::ra();
        let mut current_fp = arch::register::fp();

        log::error!("=============== BEGIN BACKTRACE ================");
        while current_pc >= _stext as usize && current_pc <= _etext as usize && current_fp != 0 {
            println!("{:#018x}", current_pc - size_of::<usize>());
            current_fp = *(current_fp as *const usize).offset(-2);
            current_pc = *(current_fp as *const usize).offset(-1);
        }
        log::error!("=============== END BACKTRACE ================");
    }
}
