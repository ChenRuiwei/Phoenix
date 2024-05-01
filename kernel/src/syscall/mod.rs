//! Implementation of syscalls

mod consts;
mod fs;
pub mod futex;
mod misc;
mod mm;
mod process;
mod resource;
mod signal;
mod time;

use ::futex::RobustListHead;
pub use consts::SyscallNo;
use fs::*;
use misc::*;
use mm::*;
pub use process::CloneFlags;
use process::*;
use resource::*;
use signal::*;
use systype::SyscallResult;
use time::*;

use crate::{
    mm::{FutexWord, UserReadPtr, UserWritePtr},
    syscall::futex::{sys_futex, sys_get_robust_list, sys_set_robust_list},
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
pub async fn syscall(syscall_no: usize, args: [usize; 6]) -> SyscallResult {
    pub use SyscallNo::*;
    let Some(syscall_no) = SyscallNo::from_repr(syscall_no) else {
        log::error!("Syscall number not included: {}", syscall_no);
        return Ok(0);
    };
    strace!("{}, args: {:?}", syscall_no, args);
    match syscall_no {
        // Process
        EXIT => sys_exit(args[0] as _),
        EXIT_GROUP => sys_exit_group(args[0] as _),
        EXECVE => sys_execve(args[0].into(), args[1].into(), args[2].into()).await,
        SCHED_YIELD => sys_sched_yield().await,
        CLONE => sys_clone(args[0], args[1], args[2], args[3], args[4]),
        WAIT4 => sys_wait4(args[0] as _, args[1].into(), args[2] as _, args[3]).await,
        GETPID => sys_getpid(),
        GETPPID => sys_getppid(),
        // Memory
        BRK => sys_brk(args[0].into()),
        MMAP => sys_mmap(
            args[0].into(),
            args[1],
            args[2] as _,
            args[3] as _,
            args[4],
            args[5],
        ),
        MUNMAP => sys_munmap(args[0].into(), args[1]),
        // File system
        READ => sys_read(args[0], args[1].into(), args[2]).await,
        WRITE => sys_write(args[0], args[1].into(), args[2]).await,
        OPENAT => sys_openat(args[0] as _, args[1].into(), args[2] as _, args[3] as _),
        CLOSE => sys_close(args[0]),
        MKDIR => sys_mkdirat(args[0] as _, args[1].into(), args[2] as _),
        GETCWD => sys_getcwd(args[0].into(), args[1]),
        CHDIR => sys_chdir(args[0].into()),
        DUP => sys_dup(args[0]),
        DUP3 => sys_dup3(args[0], args[1], args[2] as _),
        FSTAT => sys_fstat(args[0], args[1].into()),
        GETDENTS64 => sys_getdents64(args[0], args[1], args[2]),
        UNLINKAT => sys_unlinkat(args[0] as _, args[1].into(), args[2] as _),
        MOUNT => {
            sys_mount(
                args[0].into(),
                args[1].into(),
                args[2].into(),
                args[3] as _,
                args[4].into(),
            )
            .await
        }
        UMOUNT2 => sys_umount2(args[0].into(), args[1] as _).await,
        PIPE2 => sys_pipe2(args[0].into(), args[1] as _),
        // Signal
        RT_SIGPROCMASK => sys_sigprocmask(args[0], args[1].into(), args[2].into()),
        RT_SIGACTION => sys_sigaction(args[0] as _, args[1].into(), args[2].into()),
        KILL => sys_kill(args[0] as _, args[1] as _),
        TKILL => sys_tkill(args[0] as _, args[1] as _),
        TGKILL => sys_tgkill(args[0] as _, args[1] as _, args[2] as _),
        RT_SIGRETURN => sys_sigreturn(),
        RT_SIGSUSPEND => sys_sigsuspend(args[0].into()).await,
        // times
        GETTIMEOFDAY => sys_gettimeofday(args[0].into(), args[1]),
        TIMES => sys_times(args[0].into()),
        NANOSLEEP => sys_nanosleep(args[0].into(), args[1].into()).await,
        CLOCK_GETTIME => sys_clock_gettime(args[0], args[1].into()),
        CLOCK_SETTIME => sys_clock_settime(args[0], args[1].into()),
        CLOCK_GETRES => sys_clock_getres(args[0], args[1].into()),
        GETITIMER => sys_getitier(args[0] as _, args[1].into()),
        SETITIMER => sys_setitier(args[0] as _, args[1].into(), args[2].into()),
        // Futex
        FUTEX => {
            sys_futex(
                FutexWord::from(args[0]),
                args[1] as i32,
                args[2] as u32,
                args[3] as u32,
                args[4] as u32,
                args[5] as u32,
            )
            .await
        }
        GET_ROBUST_LIST => sys_get_robust_list(
            args[0] as i32,
            UserWritePtr::<RobustListHead>::from(args[1]),
            UserWritePtr::<usize>::from(args[2]),
        ),
        SET_ROBUST_LIST => {
            sys_set_robust_list(UserReadPtr::<RobustListHead>::from(args[0]), args[1])
        }
        // Miscellaneous
        UNAME => sys_uname(args[0].into()),
        GETRUSAGE => sys_getrusage(args[0] as _, args[1].into()),
        _ => {
            log::error!("Unsupported syscall: {}", syscall_no);
            Ok(0)
        }
    }
}
