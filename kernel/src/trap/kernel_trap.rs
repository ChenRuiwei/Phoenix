use irq_count::IRQ_COUNTER;
use riscv::register::{
    scause::{self, Interrupt, Trap},
    sepc, stval,
};

use crate::processor::local_hart;

/// Kernel trap handler
#[no_mangle]
pub fn kernel_trap_handler() {
    let scause = scause::read();
    let _stval = stval::read();
    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            log::info!("[kernel] receive externel interrupt");
            todo!()
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            IRQ_COUNTER.add1(1);
            todo!()
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
