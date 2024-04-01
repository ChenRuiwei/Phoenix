use arch::interrupts::{disable_interrupt, enable_interrupt};
use irq_count::IRQ_COUNTER;
use log::warn;
use memory::{VirtAddr, VA_WIDTH_SV39};
use riscv::register::{
    scause::{self, Exception, Interrupt, Trap},
    sepc,
    sstatus::FS,
    stval,
};

use super::{set_kernel_trap_entry, TrapContext};
use crate::{
    mm::memory_space,
    processor::{current_task, current_trap_cx, hart::local_hart},
    stack_trace,
    syscall::syscall,
    trap::set_user_trap_entry,
};

#[no_mangle]
/// handle an interrupt, exception, or system call from user space
pub async fn trap_handler() {
    set_kernel_trap_entry();

    let stval = stval::read();
    let scause = scause::read();

    enable_interrupt();

    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            stack_trace!();
            let mut cx = current_trap_cx();
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

    extern "C" {
        // fn __alltraps();
        fn __return_to_user(cx: *mut TrapContext);
    }

    unsafe {
        __return_to_user(current_trap_cx());
    }
}
