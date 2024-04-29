//! Implementation of syscalls

mod consts;
mod fs;
mod misc;
mod mm;
mod process;
mod resource;
mod signal;
mod time;

use ::signal::sigset::SigSet;
use ::time::{
    timespec::TimeSpec,
    timeval::{ITimerVal, TimeVal},
    tms::TMS,
};
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
        resource::{sys_getrusage, Rusage},
        signal::{sys_sigaction, sys_sigreturn},
        time::{
            sys_clock_getres, sys_clock_gettime, sys_clock_settime, sys_getitier, sys_gettimeofday,
            sys_nanosleep, sys_setitier, sys_times,
        },
    },
    task::signal::SigAction,
};

#[cfg(feature = "strace")]
pub const STRACE_COLOR_CODE: u8 = 35; // Purple

/// Syscall trace.
#[cfg(feature = "strace")]
#[macro_export]
macro_rules! strace {
    ($fmt:expr, $($args:tt)*) => {
        use $crate::{
            processor::hart::{local_hart, current_task}
        };
        $crate::impls::print_in_color(
            format_args!(concat!("[SYSCALL][H{},P{},T{}] ",  $fmt," \n"),
            local_hart().hart_id(),
            current_task().pid(),
            current_task().tid(),
            $($args)*),
            $crate::syscall::STRACE_COLOR_CODE
        );
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
                "{}, args: {:?}",
                stringify!($handler),
                $args,
            );
            $handler$args
        }
    };
    ($handler: ident, $args: tt, $await: tt) => {
        {
            strace!(
                "{}, args: {:?}",
                stringify!($handler),
                $args,
            );
            $handler$args.$await
        }
    };
}

macro_rules! asys_handler {
    ($handler: ident, $args: tt) => {
        {
            strace!(
                "{}, args: {:?}",
                stringify!($handler),
                $args,
            );
            $handler$args.await
        }
    };
}

/// Handle syscall exception with `syscall_id` and other arguments.
pub async fn syscall(syscall_id: usize, args: [usize; 6]) -> SyscallResult {
    match syscall_id {
        // Process
        SYSCALL_EXIT => sys_handler!(sys_exit, (args[0] as i32)),
        SYSCALL_EXIT_GROUP => sys_handler!(sys_exit_group, (args[0] as i32)),
        SYSCALL_EXECVE => asys_handler!(
            sys_execve,
            (
                UserReadPtr::<u8>::from(args[0]),
                UserReadPtr::<usize>::from(args[1]),
                UserReadPtr::<usize>::from(args[2]),
            )
        ),
        SYSCALL_SCHED_YIELD => asys_handler!(sys_sched_yield, ()),
        SYSCALL_CLONE => sys_handler!(sys_clone, (args[0], args[1], args[2], args[3], args[4])),
        SYSCALL_WAIT4 => asys_handler!(
            sys_wait4,
            (
                args[0] as i32,
                UserWritePtr::<i32>::from(args[1]),
                args[2] as i32,
                args[3]
            )
        ),
        SYSCALL_GETPID => sys_handler!(sys_getpid, ()),
        SYSCALL_GETPPID => sys_handler!(sys_getppid, ()),
        // Memory
        SYSCALL_BRK => sys_handler!(sys_brk, (VirtAddr::from(args[0]))),
        SYSCALL_MMAP => sys_handler!(
            sys_mmap,
            (
                VirtAddr::from(args[0]),
                args[1],
                args[2] as i32,
                args[3] as i32,
                args[4],
                args[5]
            )
        ),
        SYSCALL_MUNMAP => sys_handler!(sys_munmap, (VirtAddr::from(args[0]), args[1],)),
        // File system
        SYSCALL_READ => asys_handler!(
            sys_read,
            (args[0], UserWritePtr::<u8>::from(args[1]), args[2])
        ),
        SYSCALL_WRITE => asys_handler!(
            sys_write,
            (args[0], UserReadPtr::<u8>::from(args[1]), args[2])
        ),
        SYSCALL_OPENAT => {
            sys_handler!(
                sys_openat,
                (
                    args[0] as isize,
                    UserReadPtr::<u8>::from(args[1]),
                    args[2] as i32,
                    args[3] as u32
                )
            )
        }
        SYSCALL_CLOSE => sys_handler!(sys_close, (args[0])),
        SYSCALL_MKDIR => sys_handler!(
            sys_mkdirat,
            (
                args[0] as isize,
                UserReadPtr::<u8>::from(args[1]),
                args[2] as u32
            )
        ),
        SYSCALL_GETCWD => sys_handler!(sys_getcwd, (UserWritePtr::<u8>::from(args[0]), args[1])),
        SYSCALL_CHDIR => sys_handler!(sys_chdir, (UserReadPtr::<u8>::from(args[0]))),
        SYSCALL_DUP => sys_handler!(sys_dup, (args[0])),
        SYSCALL_DUP3 => sys_handler!(sys_dup3, (args[0], args[1], args[2] as i32)),
        SYSCALL_FSTAT => sys_handler!(sys_fstat, (args[0], UserWritePtr::<Kstat>::from(args[1]))),
        SYSCALL_GETDENTS64 => sys_handler!(sys_getdents64, (args[0], args[1], args[2])),
        SYSCALL_UNLINKAT => sys_handler!(
            sys_unlinkat,
            (
                args[0] as isize,
                UserReadPtr::<u8>::from(args[1]),
                args[2] as i32
            )
        ),
        SYSCALL_PIPE2 => sys_handler!(
            sys_pipe2,
            (UserWritePtr::<[u32; 2]>::from(args[0]), args[1] as i32)
        ),
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
                args[0] as i32,
                UserReadPtr::<SigAction>::from(args[1]),
                UserWritePtr::<SigAction>::from(args[2])
            )
        ),
        SYSCALL_KILL => sys_handler!(sys_kill, (args[0] as isize, args[1] as i32)),
        SYSCALL_TKILL => sys_handler!(sys_tkill, (args[0] as isize, args[1] as i32)),
        SYSCALL_TGKILL => sys_handler!(
            sys_tgkill,
            (args[0] as isize, args[1] as isize, args[2] as i32)
        ),
        SYSCALL_RT_SIGRETURN => sys_handler!(sys_sigreturn, ()),
        SYSCALL_RT_SIGSUSPEND => {
            asys_handler!(sys_sigsuspend, (UserReadPtr::<SigSet>::from(args[0])))
        }
        // times
        SYSCALL_GETTIMEOFDAY => sys_handler!(
            sys_gettimeofday,
            (UserWritePtr::<TimeVal>::from(args[0]), args[1])
        ),
        SYSCALL_TIMES => sys_handler!(sys_times, (UserWritePtr::<TMS>::from(args[0]))),
        SYSCALL_NANOSLEEP => asys_handler!(
            sys_nanosleep,
            (
                UserReadPtr::<TimeSpec>::from(args[0]),
                UserWritePtr::<TimeSpec>::from(args[1])
            )
        ),
        SYSCALL_CLOCK_GETTIME => sys_handler!(
            sys_clock_gettime,
            (args[0], UserWritePtr::<TimeSpec>::from(args[1]))
        ),
        SYSCALL_CLOCK_SETTIME => sys_handler!(
            sys_clock_settime,
            (args[0], UserReadPtr::<TimeSpec>::from(args[1]))
        ),
        SYSCALL_CLOCK_GETRES => sys_handler!(
            sys_clock_getres,
            (args[0], UserWritePtr::<TimeSpec>::from(args[1]))
        ),
        SYSCALL_GETITIMER => sys_handler!(
            sys_getitier,
            (args[0] as i32, UserWritePtr::<ITimerVal>::from(args[1]))
        ),
        SYSCALL_SETITIMER => sys_handler!(
            sys_setitier,
            (
                args[0] as i32,
                UserReadPtr::<ITimerVal>::from(args[1]),
                UserWritePtr::<ITimerVal>::from(args[2])
            )
        ),
        // Miscellaneous
        SYSCALL_UNAME => sys_handler!(sys_uname, (UserWritePtr::<UtsName>::from(args[0]))),
        SYSCALL_GETRUSAGE => sys_handler!(
            sys_getrusage,
            (args[0] as i32, UserWritePtr::<Rusage>::from(args[1]))
        ),
        _ => {
            error!("Unsupported syscall_id: {}", syscall_id);
            Ok(0)
        }
    }
}
