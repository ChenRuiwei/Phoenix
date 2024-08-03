//! Trap from kernel.

use arch::{
    interrupts::set_trap_handler_vector,
    time::{get_time_duration, set_next_timer_irq, set_timer_irq},
};
use memory::VirtAddr;
use riscv::register::{
    scause::{self, Exception, Interrupt, Scause, Trap},
    sepc, sstatus, stval, stvec,
};
use signal::{Sig, SigDetails, SigInfo};
use timer::TIMER_MANAGER;

use crate::{
    mm::PageFaultAccessType,
    processor::hart::{current_task_ref, local_hart},
    when_debug,
};

fn panic_on_unknown_trap() {
    panic!(
        "[kernel] sstatus sum {}, {:?}(scause:{}) in application, bad addr = {:#x}, bad instruction = {:#x}, kernel panicked!!",
        sstatus::read().sum(),
        scause::read().cause(),
        scause::read().bits(),
        stval::read(),
        sepc::read(),
    );
}

/// Kernel trap handler
#[no_mangle]
pub fn kernel_trap_handler() {
    let stval = stval::read();
    let scause = scause::read();
    let sepc = sepc::read();
    let cause = scause.cause();
    match scause.cause() {
        Trap::Interrupt(i) => match i {
            Interrupt::SupervisorExternal => {
                log::info!("[kernel] receive externel interrupt");
                driver::get_device_manager_mut().handle_irq();
            }
            Interrupt::SupervisorTimer => {
                // log::error!("[kernel_trap] receive timer interrupt");
                TIMER_MANAGER.check();
                unsafe { set_next_timer_irq() };
                #[cfg(feature = "preempt")]
                {
                    use arch::time::set_timer_irq;

                    use crate::processor::hart::local_hart;

                    if !executor::has_task() {
                        return;
                    }
                    unsafe { set_timer_irq(1) };
                    let mut old_hart = local_hart().enter_preempt_switch();
                    log::warn!("kernel preempt");
                    executor::run_one();
                    log::warn!("kernel preempt fininshed");
                    local_hart().leave_preempt_switch(&mut old_hart);
                }
            }
            _ => panic_on_unknown_trap(),
        },
        Trap::Exception(e) => match e {
            Exception::StorePageFault
            | Exception::InstructionPageFault
            | Exception::LoadPageFault => {
                log::info!(
                        "[trap_handler] encounter page fault, addr {stval:#x}, instruction {sepc:#x} scause {cause:?}",
                );
                let access_type = match e {
                    Exception::InstructionPageFault => PageFaultAccessType::RX,
                    Exception::LoadPageFault => PageFaultAccessType::RO,
                    Exception::StorePageFault => PageFaultAccessType::RW,
                    _ => unreachable!(),
                };

                let result = current_task_ref().with_mut_memory_space(|m| {
                    m.handle_page_fault(VirtAddr::from(stval), access_type)
                });
                if let Err(_e) = result {
                    log::warn!(
                        "[trap_handler] encounter page fault, addr {stval:#x}, instruction {sepc:#x} scause {cause:?}",
                    );
                    log::warn!("{:x?}", current_task_ref().trap_context_mut());
                    log::warn!("bad memory access, send SIGSEGV to task");
                    current_task_ref().receive_siginfo(
                        SigInfo {
                            sig: Sig::SIGSEGV,
                            code: SigInfo::KERNEL,
                            details: SigDetails::None,
                        },
                        false,
                    );
                }
            }
            _ => panic_on_unknown_trap(),
        },
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
