use irq_count::IRQ_COUNTER;
use riscv::register::{
    scause::{self, Interrupt, Trap},
    sepc, stval,
};

use crate::{
    processor::local_hart,
    timer::{handle_timeout_events, set_next_trigger},
};

/// Kernel trap handler
#[no_mangle]
pub fn kernel_trap_handler() {
    let scause = scause::read();
    let _stval = stval::read();
    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            log::info!("[kernel] receive externel interrupt");
            driver::intr_handler(local_hart().hart_id());
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            // log::trace!("[kernel] receive timer interrupt");
            IRQ_COUNTER.add1(1);
            handle_timeout_events();
            set_next_trigger();
        }
        _ => {
            log::error!(
                "[kernel] {:?}(scause:{}) in application, bad addr = {:#x}, bad instruction = {:#x}, kernel panicked!!",
                scause::read().cause(),
                scause::read().bits(),
                stval::read(),
                sepc::read(),
            );
            panic!(
                "a trap {:?} from kernel! stval {:#x}",
                scause::read().cause(),
                stval::read()
            );
        }
    }
}
