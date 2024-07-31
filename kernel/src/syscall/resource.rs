use config::{board::MAX_HARTS, process::USER_STACK_SIZE};
use systype::{RLimit, Rusage, SysError, SyscallResult};

use super::Syscall;
use crate::{
    mm::{UserReadPtr, UserWritePtr},
    task::TASK_MANAGER,
};

impl Syscall<'_> {
    /// getrusage() returns resource usage measures for who, which can be one of
    /// the following:
    /// - RUSAGE_SELF: Return resource usage statistics for the calling process,
    ///   which is the sum of resources used by all threads in the process.
    /// - RUSAGE_CHILDREN: Return  resource usage statistics for all children of
    ///   the calling process that have terminated and been waited for. These
    ///   statistics will include the resources used by grandchildren, and
    ///   further removed descendants, if all of the intervening descendants
    ///   waited on their terminated children.
    /// - RUSAGE_THREAD: Return resource usage statistics for the calling
    ///   thread.
    pub fn sys_getrusage(&self, who: i32, usage: UserWritePtr<Rusage>) -> SyscallResult {
        let task = self.task;
        const RUSAGE_SELF: i32 = 0;
        const RUSAGE_CHILDREN: i32 = -1;
        const RUSAGE_THREAD: i32 = 1;
        let mut ret = Rusage::default();
        match who {
            RUSAGE_SELF => {
                let (total_utime, total_stime) = task.get_process_ustime();
                ret.utime = total_utime.into();
                ret.stime = total_stime.into();
                usage.write(&task, ret)?;
            }
            RUSAGE_CHILDREN => {
                log::error!("rusage children not implemented");
                let (total_utime, total_stime) = task.get_process_ustime();
                ret.utime = total_utime.into();
                ret.stime = total_stime.into();
                usage.write(&task, ret)?;
            }
            RUSAGE_THREAD => {
                log::error!("rusage thread not implemented");
                let (total_utime, total_stime) = task.get_process_ustime();
                ret.utime = total_utime.into();
                ret.stime = total_stime.into();
                usage.write(&task, ret)?;
            }
            _ => return Err(SysError::EINVAL),
        }
        Ok(0)
    }

    pub fn sys_prlimit64(
        &self,
        pid: usize,
        resource: i32,
        new_limit: UserReadPtr<RLimit>,
        old_limit: UserWritePtr<RLimit>,
    ) -> SyscallResult {
        // This is a limit, in seconds, on the amount of CPU time that the process can
        // consume.
        const RLIMIT_CPU: i32 = 0;
        // This is the maximum size of the process stack, in bytes. Upon reaching this
        // limit, a SIGSEGV signal is generated. To handle this signal, a process must
        // employ an alternate signal stack (sigaltstack(2)).
        const RLIMIT_STACK: i32 = 3;
        // This specifies a value one greater than the maximum file descriptor number
        // that can be opened by this process.Attempts (open(2), pipe(2), dup(2),
        // etc.) to exceed this limit yield the error EMFILE.
        const RLIMIT_NOFILE: i32 = 7;
        const RLIMIT_CORE: i32 = 4;
        const RLIMIT_SIGPENDING: i32 = 11;

        let task = if pid == 0 {
            self.task.clone()
        } else if let Some(t) = TASK_MANAGER.get(pid) {
            t
        } else {
            return Err(SysError::ESRCH);
        };
        if old_limit.not_null() {
            let limit = match resource {
                RLIMIT_CPU => RLimit {
                    rlim_cur: MAX_HARTS,
                    rlim_max: MAX_HARTS,
                },
                RLIMIT_STACK => RLimit {
                    rlim_cur: USER_STACK_SIZE,
                    rlim_max: USER_STACK_SIZE,
                },
                RLIMIT_NOFILE => task.with_fd_table(|table| table.rlimit()),
                RLIMIT_CORE | RLIMIT_SIGPENDING => RLimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                },
                r => {
                    log::error!("[sys_prlimit64] get old_limit : unimplemented {r}");
                    RLimit {
                        rlim_cur: 0,
                        rlim_max: 0,
                    }
                }
            };
            old_limit.write(&task, limit)?;
        }
        if new_limit.not_null() {
            let limit = new_limit.read(&task)?;
            log::info!("[sys_prlimit64] new_limit: {limit:?}");
            match resource {
                RLIMIT_NOFILE => {
                    task.with_mut_fd_table(|table| table.set_rlimit(limit));
                }
                r => {
                    log::error!("[sys_prlimit64] set new_limit : unimplemented {r}");
                }
            }
        }

        Ok(0)
    }
}
