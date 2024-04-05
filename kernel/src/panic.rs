//! The panic handler
use core::{arch::riscv64::wfi, mem::size_of, panic::PanicInfo, sync::atomic::Ordering};

use arch::interrupts::disable_interrupt;
use driver::sbi::shutdown;

use crate::processor::hart::local_hart;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Ignore interrupts
    disable_interrupt();
    let logging_initialized = unsafe { logging::INITIALIZED.load(Ordering::SeqCst) };
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

    #[cfg(feature = "stack_trace")]
    {
        println!("backtrace:");
        crate::processor::local_hart()
            .env()
            .stack_tracker
            .print_stacks_err();
    }

    shutdown()
}

fn backtrace() {
    extern "C" {
        fn _stext();
        fn _etext();
    }
    unsafe {
        let mut current_pc = arch::register::lr();
        let mut current_fp = arch::register::fp();
        let mut stack_num = 0;

        log::error!("");
        log::error!("=============== BEGIN BACKTRACE ================");

        while current_pc >= _stext as usize && current_pc <= _etext as usize && current_fp != 0 {
            log::error!(
                "#{:02} PC: {:#018x} FP: {:#018x}",
                stack_num,
                current_pc - size_of::<usize>(),
                current_fp
            );
            stack_num += 1;
            current_fp = *(current_fp as *const usize).offset(-2);
            current_pc = *(current_fp as *const usize).offset(-1);
        }

        log::error!("=============== END BACKTRACE ================");
        log::error!("");
    }
}
