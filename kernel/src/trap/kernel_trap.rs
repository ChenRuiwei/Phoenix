use arch::interrupts::set_trap_handler_vector;
use irq_count::IRQ_COUNTER;
use riscv::register::{
    scause::{self, Interrupt, Trap},
    sepc, stval, stvec,
};

use crate::{processor::hart::local_hart, when_debug};

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
            panic!(
                "[kernel] {:?}(scause:{}) in application, bad addr = {:#x}, bad instruction = {:#x}, kernel panicked!!",
                scause::read().cause(),
                scause::read().bits(),
                stval::read(),
                sepc::read(),
            );
        }
    }
}

extern "C" {
    fn __user_rw_trap_vector();
}

#[inline(always)]
pub fn set_kernel_user_rw_trap() {
    let trap_vaddr = __user_rw_trap_vector as usize;
    set_trap_handler_vector(trap_vaddr);
    log::trace!(
        "Switch to User-RW checking mode for hart {} at STVEC: 0x{:x}",
        local_hart().hart_id(),
        trap_vaddr
    );
}

#[inline(always)]
pub fn will_read_fail(vaddr: usize) -> bool {
    when_debug!({
        let curr_stvec = stvec::read().address();
        debug_assert!(curr_stvec == __user_rw_trap_vector as usize);
    });

    extern "C" {
        fn __try_read_user(vaddr: usize) -> bool;
    }

    unsafe { __try_read_user(vaddr) }
}

#[inline(always)]
pub fn will_write_fail(vaddr: usize) -> bool {
    when_debug!({
        let curr_stvec = stvec::read().address();
        debug_assert!(curr_stvec == __user_rw_trap_vector as usize);
    });

    extern "C" {
        fn __try_write_user(vaddr: usize) -> bool;
    }
    unsafe { __try_write_user(vaddr) }
}
