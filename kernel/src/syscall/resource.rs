use config::{board::MAX_HARTS, process::USER_STACK_SIZE};
use strum::FromRepr;
use systype::{RLimit, Rusage, SysError, SyscallResult};

use super::Syscall;
use crate::{
    mm::{UserReadPtr, UserWritePtr},
    syscall::resource,
    task::TASK_MANAGER,
};

#[derive(FromRepr, Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum Resource {
    // Per-process CPU limit, in seconds.
    CPU = 0,
    // Largest file that can be created, in bytes.
    FSIZE = 1,
    // Maximum size of data segment, in bytes.
    DATA = 2,
    // Maximum size of stack segment, in bytes.
    STACK = 3,
    // Largest core file that can be created, in bytes.
    CORE = 4,
    // Largest resident set size, in bytes.
    // This affects swapping; processes that are exceeding their
    // resident set size will be more likely to have physical memory
    // taken from them.
    RSS = 5,
    // Number of processes.
    NPROC = 6,
    // Number of open files.
    NOFILE = 7,
    // Locked-in-memory address space.
    MEMLOCK = 8,
    // Address space limit.
    AS = 9,
    // Maximum number of file locks.
    LOCKS = 10,
    // Maximum number of pending signals.
    SIGPENDING = 11,
    // Maximum bytes in POSIX message queues.
    MSGQUEUE = 12,
    // Maximum nice priority allowed to raise to.
    // Nice levels 19 .. -20 correspond to 0 .. 39
    // values of this resource limit.
    NICE = 13,
    // Maximum realtime priority allowed for non-priviledged
    // processes.
    RTPRIO = 14,
    // Maximum CPU time in microseconds that a process scheduled under a real-time
    // scheduling policy may consume without making a blocking system
    // call before being forcibly descheduled.
    RTTIME = 15,
}

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
        use Resource::*;

        let task = if pid == 0 {
            self.task.clone()
        } else if let Some(t) = TASK_MANAGER.get(pid) {
            t
        } else {
            return Err(SysError::ESRCH);
        };

        let resource = Resource::from_repr(resource).ok_or(SysError::EINVAL)?;
        if old_limit.not_null() {
            let limit = match resource {
                STACK => RLimit {
                    rlim_cur: USER_STACK_SIZE,
                    rlim_max: USER_STACK_SIZE,
                },
                NOFILE => task.with_fd_table(|table| table.rlimit()),
                r => {
                    log::warn!("[sys_prlimit64] get old_limit : unimplemented {r:?}");
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
                NOFILE => {
                    task.with_mut_fd_table(|table| table.set_rlimit(limit));
                }
                r => {
                    log::warn!("[sys_prlimit64] set new_limit : unimplemented {r:?}");
                }
            }
        }
        Ok(0)
    }
}
