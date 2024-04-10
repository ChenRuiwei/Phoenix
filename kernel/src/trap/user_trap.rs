use alloc::sync::Arc;

use arch::interrupts::{disable_interrupt, enable_interrupt};
use riscv::register::{
    scause::{self, Exception, Trap},
    sepc, stval,
};

use super::{set_kernel_trap_entry, TrapContext};
use crate::{
    processor::current_trap_cx,
    syscall::syscall,
    task::{signal::do_signal, Task},
    trap::set_user_trap_entry,
};

#[no_mangle]
/// handle an interrupt, exception, or system call from user space
pub async fn trap_handler(task: Arc<Task>) {
    set_kernel_trap_entry();

    let stval = stval::read();
    let scause = scause::read();

    enable_interrupt();

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

#[no_mangle]
/// Back to user mode.
/// Note that we don't need to flush TLB since user and
/// kernel use the same pagetable.
pub fn trap_return() {
    // Important!
    disable_interrupt();

    set_user_trap_entry();

    // 当一个进程从内核模式返回到用户模式之前，
    // 内核会调用do_signal函数来检查并处理该进程的任何待处理信号
    do_signal();

    extern "C" {
        // fn __alltraps();
        fn __return_to_user(cx: *mut TrapContext);
    }

    unsafe {
        __return_to_user(current_trap_cx());
    }
}
