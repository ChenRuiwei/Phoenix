//! Trap from kernel.

use arch::{interrupts::set_trap_handler_vector, time::set_next_timer_irq};
use irq_count::IRQ_COUNTER;
use riscv::register::{
    scause::{self, Exception, Interrupt, Scause, Trap},
    sepc, stval, stvec,
};

use crate::when_debug;

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
            // log::trace!("[kernel_trap] receive timer interrupt");
            IRQ_COUNTER.add1(1);
            unsafe { set_next_timer_irq() };
            // TODO: handle time out events
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

pub unsafe fn set_kernel_user_rw_trap() {
    let trap_vaddr = __user_rw_trap_vector as usize;
    set_trap_handler_vector(trap_vaddr);
    log::trace!("[user check] switch to user rw checking mode at stvec: {trap_vaddr:#x}",);
}

pub fn will_read_fail(vaddr: usize) -> bool {
    when_debug!({
        let curr_stvec = stvec::read().address();
        debug_assert_eq!(curr_stvec, __user_rw_trap_vector as usize);
    });

    extern "C" {
        fn __try_read_user(ptr: usize) -> TryOpRet;
    }
    let try_op_ret = unsafe { __try_read_user(vaddr) };
    match try_op_ret.flag() {
        0 => false,
        _ => {
            when_debug!({
                let scause: Scause = try_op_ret.scause();
                match scause.cause() {
                    scause::Trap::Interrupt(i) => unreachable!("{:?}", i),
                    scause::Trap::Exception(e) => assert_eq!(e, Exception::LoadPageFault),
                };
            });
            true
        }
    }
}

pub fn will_write_fail(vaddr: usize) -> bool {
    when_debug!({
        let curr_stvec = stvec::read().address();
        debug_assert!(curr_stvec == __user_rw_trap_vector as usize);
    });
    extern "C" {
        fn __try_write_user(vaddr: usize) -> TryOpRet;
    }
    let try_op_ret = unsafe { __try_write_user(vaddr) };
    match try_op_ret.flag() {
        0 => false,
        _ => {
            when_debug!({
                let scause: Scause = try_op_ret.scause();
                match scause.cause() {
                    scause::Trap::Interrupt(i) => unreachable!("{:?}", i),
                    scause::Trap::Exception(e) => assert_eq!(e, Exception::StorePageFault),
                };
            });
            true
        }
    }
}

#[repr(C)]
struct TryOpRet {
    flag: usize,
    scause: usize,
}

impl TryOpRet {
    pub fn flag(&self) -> usize {
        self.flag
    }

    pub fn scause(&self) -> Scause {
        unsafe { core::mem::transmute(self.scause) }
    }
}
