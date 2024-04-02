//! Implementation of syscalls

mod id;

use core::panic;

pub use id::*;
use log::error;
use systype::SyscallRet;

use crate::{
    processor::{
        env::SumGuard,
        hart::{current_task, current_trap_cx},
    },
    stack_trace,
};

macro_rules! sys_handler {
    ($handler: ident, $args: tt) => {
        {
            $handler$args
        }
    };
    ($handler: ident, $args: tt, $await: tt) => {
        {
            $handler$args.$await
        }
    };
}

/// Handle syscall exception with `syscall_id` and other arguments.
pub async fn syscall(syscall_id: usize, args: [usize; 6]) -> SyscallRet {
    match syscall_id {
        SYSCALL_EXIT => sys_handler!(sys_exit, (args[0] as i8)),
        SYSCALL_WRITE => sys_handler!(sys_write, (args[0], args[1], args[2]), await),
        _ => {
            error!("Unsupported syscall_id: {}", syscall_id);
            Ok(0)
        }
    }
}

pub async fn sys_write(fd: usize, buf: usize, len: usize) -> SyscallRet {
    stack_trace!();
    assert!(fd == 1);
    let guard = SumGuard::new();
    let buf = unsafe { core::slice::from_raw_parts(buf as *const u8, len) };
    for b in buf {
        print!("{}", *b as char);
    }
    Ok(0)
}

pub fn sys_exit(exit_code: i8) -> SyscallRet {
    stack_trace!();
    log::info!(
        "[sys_exit]: exit code {}, sepc {:#x}",
        exit_code,
        current_trap_cx().sepc
    );
    let tid = current_task().pid();
    todo!();
    Ok(0)
}
