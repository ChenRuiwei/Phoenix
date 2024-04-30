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

/// Handle syscall exception with `syscall_id` and other arguments.
pub async fn syscall(syscall_id: usize, args: [usize; 6]) -> SyscallResult {
    match syscall_id {
        // Process
        SYSCALL_EXIT => sys_exit(args[0] as _),
        SYSCALL_EXIT_GROUP => sys_exit_group(args[0] as _),
        SYSCALL_EXECVE => sys_execve(args[0].into(), args[1].into(), args[2].into()).await,
        SYSCALL_SCHED_YIELD => sys_sched_yield().await,
        SYSCALL_CLONE => sys_clone(args[0], args[1], args[2], args[3], args[4]),
        SYSCALL_WAIT4 => sys_wait4(args[0] as _, args[1].into(), args[2] as _, args[3]).await,
        SYSCALL_GETPID => sys_getpid(),
        SYSCALL_GETPPID => sys_getppid(),
        // Memory
        SYSCALL_BRK => sys_brk(args[0].into()),
        SYSCALL_MMAP => sys_mmap(
            args[0].into(),
            args[1],
            args[2] as _,
            args[3] as _,
            args[4],
            args[5],
        ),
        SYSCALL_MUNMAP => sys_munmap(args[0].into(), args[1]),
        // File system
        SYSCALL_READ => sys_read(args[0], args[1].into(), args[2]).await,
        SYSCALL_WRITE => sys_write(args[0], args[1].into(), args[2]).await,
        SYSCALL_OPENAT => sys_openat(args[0] as _, args[1].into(), args[2] as _, args[3] as _),
        SYSCALL_CLOSE => sys_close(args[0]),
        SYSCALL_MKDIR => sys_mkdirat(args[0] as _, args[1].into(), args[2] as _),
        SYSCALL_GETCWD => sys_getcwd(args[0].into(), args[1]),
        SYSCALL_CHDIR => sys_chdir(args[0].into()),
        SYSCALL_DUP => sys_dup(args[0]),
        SYSCALL_DUP3 => sys_dup3(args[0], args[1], args[2] as _),
        SYSCALL_FSTAT => sys_fstat(args[0], args[1].into()),
        SYSCALL_GETDENTS64 => sys_getdents64(args[0], args[1], args[2]),
        SYSCALL_UNLINKAT => sys_unlinkat(args[0] as _, args[1].into(), args[2] as _),
        SYSCALL_MOUNT => {
            sys_mount(
                args[0].into(),
                args[1].into(),
                args[2].into(),
                args[3] as _,
                args[4].into(),
            )
            .await
        }
        SYSCALL_UMOUNT2 => sys_umount2(args[0].into(), args[1] as _).await,
        SYSCALL_PIPE2 => sys_pipe2(args[0].into(), args[1] as _),
        // Signal
        SYSCALL_RT_SIGPROCMASK => sys_sigprocmask(args[0], args[1].into(), args[2].into()),
        SYSCALL_RT_SIGACTION => sys_sigaction(args[0] as _, args[1].into(), args[2].into()),
        SYSCALL_KILL => sys_kill(args[0] as _, args[1] as _),
        SYSCALL_TKILL => sys_tkill(args[0] as _, args[1] as _),
        SYSCALL_TGKILL => sys_tgkill(args[0] as _, args[1] as _, args[2] as _),
        SYSCALL_RT_SIGRETURN => sys_sigreturn(),
        SYSCALL_RT_SIGSUSPEND => sys_sigsuspend(args[0].into()).await,
        // times
        SYSCALL_GETTIMEOFDAY => sys_gettimeofday(args[0].into(), args[1]),
        SYSCALL_TIMES => sys_times(args[0].into()),
        SYSCALL_NANOSLEEP => sys_nanosleep(args[0].into(), args[1].into()).await,
        SYSCALL_CLOCK_GETTIME => sys_clock_gettime(args[0], args[1].into()),
        SYSCALL_CLOCK_SETTIME => sys_clock_settime(args[0], args[1].into()),
        SYSCALL_CLOCK_GETRES => sys_clock_getres(args[0], args[1].into()),
        SYSCALL_GETITIMER => sys_getitier(args[0] as _, args[1].into()),
        SYSCALL_SETITIMER => sys_setitier(args[0] as _, args[1].into(), args[2].into()),
        // Miscellaneous
        SYSCALL_UNAME => sys_uname(args[0].into()),
        SYSCALL_GETRUSAGE => sys_getrusage(args[0] as _, args[1].into()),
        _ => {
            error!("Unsupported syscall_id: {}", syscall_id);
            Ok(0)
        }
    }
}
