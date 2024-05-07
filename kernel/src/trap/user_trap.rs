//! Trap from user.

use alloc::sync::Arc;

use arch::{
    interrupts::{disable_interrupt, enable_interrupt},
    time::{get_time_duration, set_next_timer_irq},
};
use async_utils::yield_now;
use memory::VirtAddr;
use riscv::register::{
    scause::{self, Exception, Interrupt, Trap},
    sepc, stval,
};
use timer::timer::TIMER_MANAGER;

use super::{set_kernel_trap, TrapContext};
use crate::{
    strace,
    syscall::{syscall, SyscallNo},
    task::{signal::do_signal, Task},
    trap::set_user_trap,
};

/// handle an interrupt, exception, or system call from user space
#[no_mangle]
pub async fn trap_handler(task: &Arc<Task>) {
    unsafe { set_kernel_trap() };

    log::trace!("[trap_handler] user task trap into kernel");
    let mut cx = task.trap_context_mut();
    let stval = stval::read();
    let scause = scause::read();
    let sepc = sepc::read();
    let cause = scause.cause();

    unsafe { enable_interrupt() };

    match cause {
        Trap::Exception(Exception::UserEnvCall) => {
            let syscall_no = cx.syscall_no();
            log::info!("[trap_handler] handle syscall no {syscall_no}");
            cx.set_user_pc_to_next();
            // get system call return value
            let result = syscall(syscall_no, cx.syscall_args()).await;
            // cx is changed during sys_exec, so we have to call it again
            cx = task.trap_context_mut();
            let ret = match result {
                Ok(ret) => ret,
                Err(e) => {
                    log::warn!("[trap_handler] syscall no {syscall_no} return, err {e:?}",);
                    -(e as isize) as usize
                }
            };
            log::info!("[trap_handler] handle syscall no {syscall_no} return val {ret:#x}");
            cx.set_user_a0(ret);
        }
        Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            log::debug!(
                "[trap_handler] encounter page fault, addr {stval:#x}, instruction {sepc:#x} scause {cause:?}",
            );
            // There are serveral kinds of page faults:
            // 1. mmap area
            // 2. sbrk area
            // 3. fork cow area
            // 4. user stack
            // 5. execve elf file
            // 6. dynamic link
            // 7. illegal page fault

            let result = task.with_mut_memory_space(|m| m.handle_page_fault(VirtAddr::from(stval)));
            if let Err(_e) = result {
                // task.with_memory_space(|m| m.print_all());
                task.set_zombie();
            }
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            log::warn!(
                "[trap_handler] detected illegal instruction, stval {stval:#x}, sepc {sepc:#x}",
            );
            task.set_zombie();
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            log::trace!("[trap_handler] timer interrupt, sepc {sepc:#x}");
            TIMER_MANAGER.check(get_time_duration());
            unsafe { set_next_timer_irq() };
            yield_now().await;
        }
        _ => {
            panic!(
                "[trap_handler] Unsupported trap {cause:?}, stval = {stval:#x}!, sepc = {sepc:#x}"
            );
        }
    }
}

/// Trap return to user mode.
#[no_mangle]
pub fn trap_return(task: &Arc<Task>) {
    extern "C" {
        fn __return_to_user(cx: *mut TrapContext);
    }

    do_signal().expect("do signal error");

    log::info!("[kernel] trap return to user...");
    unsafe {
        disable_interrupt();
        set_user_trap()
        // WARN: stvec can not be changed below. One hidden mistake is to use
        // `UserPtr` implicitly which will change stvec to `__trap_from_kernel`.
    };
    task.time_stat().record_trap_return();
    unsafe {
        __return_to_user(task.trap_context_mut());
        // NOTE: next time when user traps into kernel, it will come back here
        // and return to `user_loop` function.
    }
    task.time_stat().record_trap();
}
