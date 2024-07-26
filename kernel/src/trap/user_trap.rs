//! Trap from user.

use alloc::sync::Arc;

use arch::{
    interrupts::{disable_interrupt, enable_interrupt},
    time::{get_time_duration, set_next_timer_irq},
};
use async_utils::yield_now;
use executor::has_task;
use memory::VirtAddr;
use riscv::register::{
    scause::{self, Exception, Interrupt, Trap},
    sepc,
    sstatus::FS,
    stval,
};
use signal::{Sig, SigDetails, SigInfo};
use systype::SysError;
use timer::TIMER_MANAGER;

use super::{set_kernel_trap, TrapContext};
use crate::{mm::PageFaultAccessType, syscall::Syscall, task::Task, trap::set_user_trap};

/// handle an interrupt, exception, or system call from user space
/// return if it is syscall and has been interrupted
#[no_mangle]
pub async fn trap_handler(task: &Arc<Task>) -> bool {
    unsafe { set_kernel_trap() };

    let mut cx = task.trap_context_mut();
    let stval = stval::read();
    let scause = scause::read();
    let sepc = sepc::read();
    let cause = scause.cause();
    log::trace!("[trap_handler] user task trap into kernel");
    log::trace!("[trap_handler] sepc:{sepc:#x}, stval:{stval:#x}");
    unsafe { enable_interrupt() };

    // if task.time_stat_ref().need_schedule() && executor::has_task() {
    //     log::info!("time slice used up, yield now");
    //     yield_now().await;
    // }

    match cause {
        Trap::Exception(e) => {
            match e {
                Exception::UserEnvCall => {
                    let syscall_no = cx.syscall_no();
                    cx.set_user_pc_to_next();
                    // get system call return value
                    let ret = Syscall::new(task)
                        .syscall(syscall_no, cx.syscall_args())
                        .await;
                    cx.save_last_user_a0();
                    cx.set_user_a0(ret);
                    if ret == -(SysError::EINTR as isize) as usize {
                        return true;
                    }
                }
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
                    // There are serveral kinds of page faults:
                    // 1. mmap area
                    // 2. sbrk area
                    // 3. fork cow area
                    // 4. user stack
                    // 5. execve elf file
                    // 6. dynamic link
                    // 7. illegal page fault

                    let result = task.with_mut_memory_space(|m| {
                        m.handle_page_fault(VirtAddr::from(stval), access_type)
                    });
                    if let Err(_e) = result {
                        log::warn!(
                            "[trap_handler] encounter page fault, addr {stval:#x}, instruction {sepc:#x} scause {cause:?}",
                        );
                        // backtrace::backtrace();
                        log::warn!("{:x?}", task.trap_context_mut());
                        // task.with_memory_space(|m| m.print_all());
                        log::warn!("bad memory access, send SIGSEGV to task");
                        task.receive_siginfo(
                            SigInfo {
                                sig: Sig::SIGSEGV,
                                code: SigInfo::KERNEL,
                                details: SigDetails::None,
                            },
                            false,
                        );
                        // task.set_zombie();
                    }
                }
                Exception::IllegalInstruction => {
                    log::warn!(
                        "[trap_handler] detected illegal instruction, stval {stval:#x}, sepc {sepc:#x}",
                    );
                    task.set_zombie();
                }
                e => {
                    log::warn!("Unknown user exception: {:?}", e);
                }
            }
        }
        Trap::Interrupt(i) => {
            match i {
                Interrupt::SupervisorTimer => {
                    // NOTE: user may trap into kernel frequently, as a consequence, this timer are
                    // likely not triggered in user mode but rather be triggered in supervisor mode,
                    // which will cause user program running on the cpu for a long time.
                    log::trace!("[trap_handler] timer interrupt, sepc {sepc:#x}");
                    TIMER_MANAGER.check(get_time_duration());
                    unsafe { set_next_timer_irq() };
                    if executor::has_task() {
                        yield_now().await;
                    }
                }
                Interrupt::SupervisorExternal => {
                    log::info!("[kernel] receive externel interrupt");
                    driver::get_device_manager_mut().handle_irq();
                }
                _ => {
                    panic!(
                    "[trap_handler] Unsupported trap {cause:?}, stval = {stval:#x}!, sepc = {sepc:#x}"
                    );
                }
            }
        }
    }
    false
}

extern "C" {
    fn __return_to_user(cx: *mut TrapContext);
}

/// Trap return to user mode.
#[no_mangle]
pub fn trap_return(task: &Arc<Task>) {
    log::info!("[kernel] trap return to user...");
    unsafe {
        disable_interrupt();
        set_user_trap()
        // WARN: stvec can not be changed below. One hidden mistake is to use
        // `UserPtr` implicitly which will change stvec to `__trap_from_kernel`.
    };
    task.time_stat().record_trap_return();

    // Restore the float regs if needed.
    // Two cases that may need to restore regs:
    // 1. This task has yielded after last trap
    // 2. This task encounter a signal handler
    task.trap_context_mut().user_fx.restore();
    task.trap_context_mut().sstatus.set_fs(FS::Clean);
    unsafe {
        __return_to_user(task.trap_context_mut());
        // NOTE: next time when user traps into kernel, it will come back here
        // and return to `user_loop` function.
    }
    task.trap_context_mut()
        .user_fx
        .mark_save_if_needed(task.trap_context_mut().sstatus);
    task.time_stat().record_trap();
}
