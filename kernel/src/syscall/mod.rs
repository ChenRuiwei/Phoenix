//! Implementation of syscalls

mod consts;
mod fs;
mod misc;
mod mm;
mod process;
mod signal;
mod time;

use ::signal::sigset::SigSet;
use ::time::{timespec::TimeSpec, timeval::TimeVal, tms::TMS};
use consts::*;
use fs::*;
use log::error;
use memory::VirtAddr;
use misc::*;
use mm::*;
pub use process::CloneFlags;
use process::*;
use signal::*;
use systype::SyscallResult;

use crate::{
    mm::{UserReadPtr, UserWritePtr},
    syscall::{
        misc::UtsName,
        signal::{sys_sigaction, sys_sigreturn},
        time::{sys_gettimeofday, sys_nanosleep, sys_times},
    },
    task::signal::SigAction,
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
                crate::processor::current_task().trap_context_mut().sepc
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
                crate::processor::current_task().trap_context_mut().sepc
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
        SYSCALL_GETPID => sys_handler!(sys_getpid, ()),
        SYSCALL_GETPPID => sys_handler!(sys_getppid, ()),
        // Memory
        SYSCALL_BRK => {
            sys_handler!(sys_brk, (VirtAddr::from(args[0])))
        }
        // File system
        SYSCALL_WRITE => {
            sys_handler!(sys_write, (args[0], UserReadPtr::<u8>::from(args[1]), args[2]), await)
        }

        // Signal
        SYSCALL_RT_SIGPROCMASK => sys_handler!(
            sys_sigprocmask,
            (
                args[0],
                UserReadPtr::<SigSet>::from(args[1]),
                UserWritePtr::<SigSet>::from(args[2]),
            )
        ),

        SYSCALL_RT_SIGACTION => sys_handler!(
            sys_sigaction,
            (
                args[0],
                UserReadPtr::<SigAction>::from(args[1]),
                UserWritePtr::<SigAction>::from(args[2])
            )
        ),
        SYSCALL_RT_SIGRETURN => sys_handler!(sys_sigreturn, ()),
        SYSCALL_GETTIMEOFDAY => sys_handler!(
            sys_gettimeofday,
            (UserWritePtr::<TimeVal>::from(args[0]), args[1])
        ),
        SYSCALL_TIMES => sys_handler!(sys_times, (UserWritePtr::<TMS>::from(args[0]))),
        SYSCALL_NANOSLEEP => sys_handler!(
            sys_nanosleep,
            (
                UserReadPtr::<TimeSpec>::from(args[1]),
                UserWritePtr::<TimeSpec>::from(args[2])
            ),
            await
        ),
        // Miscellaneous
        SYSCALL_UNAME => sys_handler!(sys_uname, (UserWritePtr::<UtsName>::from(args[0]))),
        _ => {
            error!("Unsupported syscall_id: {}", syscall_id);
            Ok(0)
        }
    }
}
