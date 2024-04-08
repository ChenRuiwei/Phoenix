use alloc::sync::Arc;

use arch::interrupts::{disable_interrupt, enable_interrupt};
use riscv::register::{
    scause::{self, Exception, Trap},
    sepc, stval,
};

use super::{set_kernel_trap, TrapContext};
use crate::{processor::current_trap_cx, syscall::syscall, task::Task, trap::set_user_trap};

#[no_mangle]
/// handle an interrupt, exception, or system call from user space
pub async fn trap_handler(task: Arc<Task>) {
    unsafe { set_kernel_trap() };

    let stval = stval::read();
    let scause = scause::read();

    unsafe { enable_interrupt() };

    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            let mut cx = task.trap_context_mut();
            cx.sepc += 4;
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
            cx.user_x[10] = match result {
                Ok(ret) => ret as usize,
                Err(err) => {
                    log::warn!(
                        "[trap_handler] syscall {} return, err {:?}",
                        cx.user_x[17],
                        err
                    );
                    -(err as isize) as usize
                }
            };
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

/// Back to user mode.
/// Note that we don't need to flush TLB since user and
/// kernel use the same pagetable.
#[no_mangle]
pub fn trap_return() {
    // Important!
    unsafe {
        disable_interrupt();
        set_user_trap()
    };

    extern "C" {
        // fn __alltraps();
        fn __return_to_user(cx: *mut TrapContext);
    }

    unsafe {
        __return_to_user(current_trap_cx());
    }
}
