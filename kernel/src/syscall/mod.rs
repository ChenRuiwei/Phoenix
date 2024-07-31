//! Implementation of syscalls

mod consts;
mod fs;
pub mod futex;
mod io;
mod misc;
mod mm;
mod net;
mod process;
mod resource;
mod sched;
mod signal;
mod time;

use alloc::sync::Arc;

pub use consts::SyscallNo;
pub use mm::MmapFlags;
pub use process::CloneFlags;
use systype::SyscallResult;

use crate::task::Task;

#[cfg(feature = "strace")]
pub const STRACE_COLOR_CODE: logging::ColorCode = logging::ColorCode::BrightMagenta;

/// Syscall trace.
// TODO: syscall trace with exact args and return value
#[cfg(feature = "strace")]
#[macro_export]
macro_rules! strace {
    ($fmt:expr, $($args:tt)*) => {
        use $crate::{
            processor::hart::{local_hart, current_task_ref}
        };
        $crate::impls::print_in_color(
            format_args!(concat!("[SYSCALL][H{},P{},T{}] ",  $fmt," \n"),
            local_hart().hart_id(),
            current_task_ref().pid(),
            current_task_ref().tid(),
            $($args)*),
            $crate::syscall::STRACE_COLOR_CODE as u8
        );
    }
}

#[cfg(not(feature = "strace"))]
#[macro_export]
macro_rules! strace {
    ($fmt:literal $(, $($arg:tt)+)?) => {};
}

pub struct Syscall<'a> {
    task: &'a Arc<Task>,
}

impl<'a> Syscall<'a> {
    pub fn new(task: &'a Arc<Task>) -> Self {
        Self { task }
    }

    /// Handle syscall exception with `syscall_id` and other arguments.
    pub async fn syscall(&self, syscall_no: usize, args: [usize; 6]) -> usize {
        use SyscallNo::*;
        let Some(syscall_no) = SyscallNo::from_repr(syscall_no) else {
            log::error!("Syscall number not included: {syscall_no}");
            unimplemented!()
        };
        // Use STRACE=1 instead
        // log::debug!("[syscall] handle {syscall_no}");
        strace!(
            "{}, args: [{:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}]",
            syscall_no,
            args[0],
            args[1],
            args[2],
            args[3],
            args[4],
            args[5]
        );
        let result = match syscall_no {
            // Process
            EXIT => self.sys_exit(args[0] as _),
            EXIT_GROUP => self.sys_exit_group(args[0] as _),
            EXECVE => {
                self.sys_execve(args[0].into(), args[1].into(), args[2].into())
                    .await
            }
            SCHED_YIELD => self.sys_sched_yield().await,
            CLONE => self.sys_clone(
                args[0],
                args[1].into(),
                args[2].into(),
                args[3].into(),
                args[4].into(),
            ),
            WAIT4 => {
                self.sys_wait4(args[0] as _, args[1].into(), args[2] as _, args[3])
                    .await
            }
            GETTID => self.sys_gettid(),
            GETPID => self.sys_getpid(),
            GETPPID => self.sys_getppid(),
            GETPGID => self.sys_getpgid(args[0]),
            SET_TID_ADDRESS => self.sys_set_tid_address(args[0]),
            GETUID => self.sys_getuid(),
            GETEUID => self.sys_geteuid(),
            SETSID => self.sys_setsid(),
            GETEGID => self.sys_do_nothing("getegid"),
            SETPGID => self.sys_setpgid(args[0], args[1]),
            GETGID => self.sys_do_nothing("getgid"),
            // Memory
            BRK => self.sys_brk(args[0].into()),
            MMAP => self.sys_mmap(
                args[0].into(),
                args[1],
                args[2] as _,
                args[3] as _,
                args[4],
                args[5],
            ),
            MUNMAP => self.sys_munmap(args[0].into(), args[1]),
            MPROTECT => self.sys_mprotect(args[0].into(), args[1], args[2] as _),
            MSYNC => self.sys_do_nothing("msync"),
            MEMBARRIER => self.sys_do_nothing("membarrier"),
            MADVISE => self.sys_do_nothing("madvise"),
            // Shared Memory
            SHMGET => self.sys_shmget(args[0], args[1], args[2] as _),
            SHMAT => self.sys_shmat(args[0], args[1].into(), args[2] as _),
            SHMDT => self.sys_shmdt(args[0].into()),
            SHMCTL => self.sys_shmctl(args[0], args[1] as _, args[2]),
            // File system
            READ => self.sys_read(args[0], args[1].into(), args[2]).await,
            WRITE => self.sys_write(args[0], args[1].into(), args[2]).await,
            PREAD64 => {
                self.sys_pread64(args[0], args[1].into(), args[2], args[3])
                    .await
            }
            PWRITE64 => {
                self.sys_pwrite64(args[0], args[1].into(), args[2], args[3])
                    .await
            }
            OPENAT => {
                self.sys_openat(args[0].into(), args[1].into(), args[2] as _, args[3] as _)
                    .await
            }
            CLOSE => self.sys_close(args[0]),
            MKDIRAT => self.sys_mkdirat(args[0].into(), args[1].into(), args[2] as _),
            GETCWD => self.sys_getcwd(args[0].into(), args[1]),
            CHDIR => self.sys_chdir(args[0].into()),
            DUP => self.sys_dup(args[0]),
            DUP3 => self.sys_dup3(args[0], args[1], args[2] as _),
            FSTAT => self.sys_fstat(args[0], args[1].into()),
            FSTATAT => {
                self.sys_fstatat(args[0].into(), args[1].into(), args[2].into(), args[3] as _)
            }
            GETDENTS64 => self.sys_getdents64(args[0], args[1], args[2]),
            UNLINKAT => self.sys_unlinkat(args[0].into(), args[1].into(), args[2] as _),
            MOUNT => {
                self.sys_mount(
                    args[0].into(),
                    args[1].into(),
                    args[2].into(),
                    args[3] as _,
                    args[4].into(),
                )
                .await
            }
            UMOUNT2 => self.sys_umount2(args[0].into(), args[1] as _).await,
            PIPE2 => self.sys_pipe2(args[0].into(), args[1] as _),
            IOCTL => self.sys_ioctl(args[0], args[1], args[2]),
            FCNTL => self.sys_fcntl(args[0], args[1] as _, args[2]),
            WRITEV => self.sys_writev(args[0], args[1].into(), args[2]).await,
            READV => self.sys_readv(args[0], args[1].into(), args[2]).await,
            SENDFILE => {
                self.sys_sendfile(args[0], args[1], args[2].into(), args[3])
                    .await
            }
            FACCESSAT => self.sys_faccessat(args[0].into(), args[1].into(), args[2], args[3]),
            LSEEK => self.sys_lseek(args[0], args[1] as _, args[2]),
            UMASK => self.sys_umask(args[0] as _),
            UTIMENSAT => {
                self.sys_utimensat(args[0].into(), args[1].into(), args[2].into(), args[3] as _)
            }
            RENAMEAT2 => self.sys_renameat2(
                args[0].into(),
                args[1].into(),
                args[2].into(),
                args[3].into(),
                args[4] as _,
            ),
            STATFS => self.sys_statfs(args[0].into(), args[1].into()),
            READLINKAT => {
                self.sys_readlinkat(args[0].into(), args[1].into(), args[2].into(), args[3])
                    .await
            }
            SYNC => self.sys_do_nothing("sync"),
            FSYNC => self.sys_do_nothing("fsync"),
            FTRUNCATE => self.sys_ftruncate(args[0], args[1] as _).await,
            FCHMODAT => self.sys_fchmodat(),
            FCHOWNAT => self.sys_do_nothing("fchownat"),
            FALLOCATE => self.sys_do_nothing("fallocate"),
            // IO
            PPOLL => {
                self.sys_ppoll(args[0].into(), args[1], args[2].into(), args[3].into())
                    .await
            }
            PSELECT6 => {
                self.sys_pselect6(
                    args[0] as _,
                    args[1].into(),
                    args[2].into(),
                    args[3].into(),
                    args[4].into(),
                    args[5].into(),
                )
                .await
            }
            // Signal
            RT_SIGPROCMASK => {
                self.sys_rt_sigprocmask(args[0], args[1].into(), args[2].into(), args[3])
            }
            RT_SIGACTION => self.sys_rt_sigaction(args[0] as _, args[1].into(), args[2].into()),
            KILL => self.sys_kill(args[0] as _, args[1] as _),
            TKILL => self.sys_tkill(args[0] as _, args[1] as _),
            TGKILL => self.sys_tgkill(args[0] as _, args[1] as _, args[2] as _),
            RT_SIGRETURN => self.sys_rt_sigreturn(),
            RT_SIGSUSPEND => self.sys_rt_sigsuspend(args[0].into()).await,
            RT_SIGTIMEDWAIT => {
                self.sys_rt_sigtimedwait(args[0].into(), args[1].into(), args[2].into())
                    .await
            }
            // Times
            GETTIMEOFDAY => self.sys_gettimeofday(args[0].into(), args[1]),
            TIMES => self.sys_times(args[0].into()),
            NANOSLEEP => self.sys_nanosleep(args[0].into(), args[1].into()).await,
            CLOCK_GETTIME => self.sys_clock_gettime(args[0], args[1].into()),
            CLOCK_SETTIME => self.sys_clock_settime(args[0], args[1].into()),
            CLOCK_GETRES => self.sys_clock_getres(args[0], args[1].into()),
            GETITIMER => self.sys_getitimer(args[0] as _, args[1].into()),
            SETITIMER => self.sys_setitimer(args[0] as _, args[1].into(), args[2].into()),
            CLOCK_NANOSLEEP => {
                self.sys_clock_nanosleep(args[0], args[1], args[2].into(), args[3].into())
                    .await
            }
            // Futex
            FUTEX => {
                self.sys_futex(
                    args[0].into(),
                    args[1] as _,
                    args[2] as _,
                    args[3] as _,
                    args[4] as _,
                    args[5] as _,
                )
                .await
            }
            GET_ROBUST_LIST => {
                self.sys_get_robust_list(args[0] as _, args[1].into(), args[2].into())
            }
            SET_ROBUST_LIST => self.sys_set_robust_list(args[0].into(), args[1]),
            // Schedule
            SCHED_SETSCHEDULER => self.sys_sched_setscheduler(),
            SCHED_GETSCHEDULER => self.sys_sched_getscheduler(),
            SCHED_GETPARAM => self.sys_sched_getparam(),
            SCHED_SETAFFINITY => self.sys_sched_setaffinity(args[0], args[1], args[2].into()),
            SCHED_GETAFFINITY => self.sys_sched_getaffinity(args[0], args[1], args[2].into()),
            // Resource
            GETRUSAGE => self.sys_getrusage(args[0] as _, args[1].into()),
            PRLIMIT64 => self.sys_prlimit64(args[0], args[1] as _, args[2].into(), args[3].into()),
            // Net
            SOCKET => self.sys_socket(args[0], args[1] as _, args[2]),
            BIND => self.sys_bind(args[0], args[1], args[2]),
            LISTEN => self.sys_listen(args[0], args[1]),
            ACCEPT => self.sys_accept(args[0], args[1], args[2].into()).await,
            CONNECT => self.sys_connect(args[0], args[1], args[2]).await,
            GETSOCKNAME => self.sys_getsockname(args[0], args[1], args[2]),
            GETPEERNAME => self.sys_getpeername(args[0], args[1], args[2].into()),
            SENDTO => {
                self.sys_sendto(args[0], args[1].into(), args[2], args[3], args[4], args[5])
                    .await
            }
            RECVFROM => {
                self.sys_recvfrom(args[0], args[1].into(), args[2], args[3], args[4], args[5])
                    .await
            }
            SETSOCKOPT => self.sys_setsockopt(args[0], args[1], args[2], args[3], args[4]),
            GETSOCKOPT => self.sys_getsockopt(args[0], args[1], args[2], args[3], args[4]),
            SHUTDOWN => self.sys_shutdown(args[0], args[1]),
            SOCKETPAIR => self.sys_socketpair(args[0], args[1], args[2], args[3].into()),
            // Miscellaneous
            UNAME => self.sys_uname(args[0].into()),
            SYSLOG => self.sys_syslog(args[0], args[1].into(), args[2]),
            SYSINFO => self.sys_sysinfo(args[0].into()),
            PERSONALITY => self.sys_do_nothing("personality"),
            _ => {
                log::error!("Unsupported syscall: {}", syscall_no);
                Ok(0)
            }
        };
        match result {
            Ok(ret) => {
                log::info!("[syscall] {syscall_no} return val {ret:#x}");
                ret
            }
            Err(e) => {
                log::warn!("[syscall] {syscall_no} return err {e:?}");
                -(e as isize) as usize
            }
        }
    }

    fn sys_do_nothing(&self, name: &str) -> SyscallResult {
        log::error!(
            "Not implemented syscall that specified to do nothing ({})",
            name
        );
        Ok(0)
    }
}
