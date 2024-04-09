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

use self::misc::sys_uname;
use crate::processor::{
    env::SumGuard,
    hart::{current_task, current_trap_cx},
};

#[cfg(feature = "strace")]
pub const STRACE_COLOR_CODE: u8 = 35; // Purple

/// Syscall trace
#[cfg(feature = "strace")]
#[macro_export]
macro_rules! strace {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        use $crate::{
            processor::{local_hart, current_task}
        };
        $crate::impls::print_in_color(
            format_args!(concat!("[SYSCALL][{},{}] ", $fmt, "\n"),
            local_hart().hart_id(),
            current_task().pid()
            $(, $($arg)+)?),
            $crate::syscall::STRACE_COLOR_CODE);
    }
}
#[cfg(not(feature = "strace"))]
#[macro_export]
macro_rules! strace {
    ($fmt:literal $(, $($arg:tt)+)?) => {};
}

macro_rules! sys_handler {
    ($handler: ident, $args: tt) => {
        {
            strace!(
                "{}, args: {:?}, sepc: {:#x}",
                stringify!($handler),
                $args,
                crate::processor::current_trap_cx().sepc
            );
            $handler$args
        }
    };
    ($handler: ident, $args: tt, $await: tt) => {
        {
            strace!(
                "{}, args: {:?}, sepc: {:#x}",
                stringify!($handler),
                $args,
                crate::processor::current_trap_cx().sepc
            );
            $handler$args.$await
        }
    };
}

/// Handle syscall exception with `syscall_id` and other arguments.
pub async fn syscall(syscall_id: usize, args: [usize; 6]) -> SyscallResult {
    match syscall_id {
        // Process
        SYSCALL_EXIT => sys_handler!(sys_exit, (args[0] as i32)),
        SYSCALL_EXECVE => sys_handler!(sys_execve, (args[0], args[1], args[2])),
        // File system
        SYSCALL_WRITE => sys_handler!(sys_write, (args[0], args[1], args[2]), await),
        // Miscellaneous
        SYSCALL_UNAME => sys_handler!(sys_uname, (args[0])),
        _ => {
            error!("Unsupported syscall_id: {}", syscall_id);
            Ok(0)
        }
    }
}
