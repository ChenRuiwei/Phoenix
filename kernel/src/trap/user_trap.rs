//! Trap from user.

use alloc::sync::Arc;

use arch::{
    interrupts::{disable_interrupt, enable_interrupt},
    time::set_next_timer_irq,
};
use riscv::register::{
    scause::{self, Exception, Interrupt, Trap},
    sepc, sstatus, stval,
};

use super::{set_kernel_trap, TrapContext};
use crate::{
    processor::{current_trap_cx, hart::current_task},
    syscall::syscall,
    task::{signal::do_signal, yield_now, Task},
    trap::set_user_trap,
};

/// handle an interrupt, exception, or system call from user space
#[no_mangle]
pub async fn trap_handler(task: Arc<Task>) {
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
            cx.set_user_a0(ret);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            log::warn!(
                "[trap_handler] detected illegal instruction, stval {stval:#x}, sepc {sepc:#x}",
            );
            // TODO: kill the process
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            log::trace!("[trap_handler] timer interrupt, sepc {sepc:#x}");
            // TODO: handle timeout events
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
pub fn trap_return() {
    unsafe {
        disable_interrupt();
        set_user_trap()
    };

    do_signal();

    extern "C" {
        fn __return_to_user(cx: *mut TrapContext);
    }

    
    // current_task().time_stat.get()
    //         .record_trap_return_time(get_time_duration());
    current_task().get_time_stat().record_trap_return_time(get_time_duration());
    unsafe {
        __return_to_user(current_trap_cx());
        // NOTE: next time when user traps into kernel, it will come back here
        // and return to `user_loop` function.
    }
    current_task().get_time_stat().record_trap_time(get_time_duration());
}
