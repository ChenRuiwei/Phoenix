//! Implementation of syscalls

mod fs;
mod id;
mod misc;
mod process;

use core::panic;

use fs::*;
use id::*;
use log::error;
use process::*;
use systype::SyscallResult;

use crate::processor::{
    env::SumGuard,
    hart::{current_task, current_trap_cx},
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
pub async fn syscall(syscall_id: usize, args: [usize; 6]) -> SyscallResult {
    match syscall_id {
        SYSCALL_EXIT => sys_handler!(sys_exit, (args[0] as i32)),
        SYSCALL_WRITE => sys_handler!(sys_write, (args[0], args[1], args[2]), await),
        _ => {
            error!("Unsupported syscall_id: {}", syscall_id);
            Ok(0)
        }
    }
}
