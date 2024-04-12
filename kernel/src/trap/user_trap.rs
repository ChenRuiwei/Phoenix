//! Trap from user.

use alloc::sync::Arc;

use arch::interrupts::{disable_interrupt, enable_interrupt};
use riscv::register::{
    scause::{self, Exception, Trap},
    sepc, stval,
};

use super::{set_kernel_trap, TrapContext};
use crate::{
    processor::current_trap_cx,
    syscall::syscall,
    task::{signal::do_signal, Task},
    trap::set_user_trap,
};

/// handle an interrupt, exception, or system call from user space
#[no_mangle]
pub async fn trap_handler(task: Arc<Task>) {
    unsafe { set_kernel_trap() };

    let stval = stval::read();
    let scause = scause::read();

    unsafe { enable_interrupt() };

    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            let mut cx = task.trap_context_mut();
            cx.set_user_pc_to_next();
            // get system call return value
            let result = syscall(
                cx.user_x[17],
                [
                    cx.user_x[10],
                    cx.user_x[11],
                    cx.user_x[12],
                    cx.user_x[13],
                    cx.user_x[14],
                    cx.user_x[15],
                ],
            )
            .await;

            // cx is changed during sys_exec, so we have to call it again
            cx = current_trap_cx();
            let ret = match result {
                Ok(ret) => ret,
                Err(err) => {
                    log::warn!(
                        "[trap_handler] syscall {} return, err {:?}",
                        cx.syscall_no(),
                        err
                    );
                    -(err as isize) as usize
                }
            };
            cx.set_user_a0(ret);
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!, sepc = {:#x}",
                scause.cause(),
                stval,
                sepc::read(),
            );
        }
    }
}

/// Trap return to user mode.
///
/// Note that we don't need to flush TLB since user and
/// kernel uses the same pagetable.
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

    unsafe {

        __return_to_user(current_trap_cx());
        // next time when user traps into kernel, it will come back here and
        // return to `user_loop` function.
    }
}
