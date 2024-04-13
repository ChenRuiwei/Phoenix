//! Implementation of syscalls

mod fs;
mod id;
mod misc;
mod mm;
mod process;
mod signal;

use core::panic;

use ::signal::sigset::SigSet;
use fs::*;
use id::*;
use log::error;
use mm::*;
pub use process::CloneFlags;
use process::*;
use systype::SyscallResult;

use self::{misc::sys_uname, signal::sys_sigprocmask};
use crate::{
    mm::{UserReadPtr, UserWritePtr},
    processor::{
        env::SumGuard,
        hart::{current_task, current_trap_cx},
    },
    syscall::misc::UtsName,
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
        SYSCALL_EXIT_GROUP => sys_handler!(sys_exit_group, (args[0] as i32)),
        SYSCALL_EXECVE => sys_handler!(
            sys_execve,
            (
                UserReadPtr::<u8>::from(args[0]),
                UserReadPtr::<usize>::from(args[1]),
                UserReadPtr::<usize>::from(args[2]),
            )
        ),
        SYSCALL_SCHED_YIELD => sys_handler!(sys_sched_yield, (), await),
        SYSCALL_CLONE => sys_handler!(sys_clone, (args[0], args[1], args[2], args[3], args[4])),
        SYSCALL_WAIT4 => sys_handler!(
            sys_wait4,
            (
                args[0] as i32,
                UserWritePtr::<i32>::from(args[1]),
                args[2] as i32,
                args[3]
            ), await
        ),
        // File system
        SYSCALL_WRITE => {
            sys_handler!(sys_write, (args[0], UserReadPtr::<u8>::from(args[1]), args[2]), await)
        }
        // Miscellaneous
        SYSCALL_UNAME => sys_handler!(sys_uname, (UserWritePtr::<UtsName>::from(args[0]))),
        SYSCALL_RT_SIGPROCMASK => {
            sys_handler!(
                sys_sigprocmask,
                (
                    args[0],
                    UserReadPtr::<SigSet>::from(args[1]),
                    UserWritePtr::<SigSet>::from(args[2]),
                )
            )
        }
        _ => {
            error!("Unsupported syscall_id: {}", syscall_id);
            Ok(0)
        }
    }
}
