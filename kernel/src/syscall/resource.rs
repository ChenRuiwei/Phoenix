use core::time::Duration;

use systype::{SysError, SyscallResult};
use time::timeval::TimeVal;

use crate::{mm::UserWritePtr, processor::hart::current_task};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Rusage {
    pub utime: TimeVal, // This is the total amount of time spent executing in user mode
    pub stime: TimeVal, // This is the total amount of time spent executing in kernel mode
    pub maxrss: usize,  // maximum resident set size
    pub ixrss: usize,   // In modern systems, this field is usually no longer used
    pub idrss: usize,   // In modern systems, this field is usually no longer used
    pub isrss: usize,   // In modern systems, this field is usually no longer used
    pub minflt: usize,  // page reclaims (soft page faults)
    pub majflt: usize,  // page faults (hard page faults)
    pub nswap: usize,   // swaps
    pub inblock: usize, // block input operations
    pub oublock: usize, // block output operations
    pub msgsnd: usize,  // In modern systems, this field is usually no longer used
    pub msgrcv: usize,  // In modern systems, this field is usually no longer used
    pub nsignals: usize, // In modern systems, this field is usually no longer used
    pub nvcsw: usize,   // voluntary context switches
    pub nivcsw: usize,  // involuntary context switches
}

/// getrusage() returns resource usage measures for who, which can be one of the
/// following:
/// - RUSAGE_SELF: Return resource usage statistics for the calling process,
///   which is the sum of resources used by all threads in the process.
/// - RUSAGE_CHILDREN: Return  resource usage statistics for all children of the
///   calling process that have terminated and been waited for.  These
///   statistics will include the resources used by grandchildren, and further
///   removed descendants, if all of  the  intervening  descendants waited on
///   their terminated children.
/// - RUSAGE_THREAD: Return  resource usage statistics for the calling thread.
pub fn sys_getrusage(who: i32, usage: UserWritePtr<Rusage>) -> SyscallResult {
    const RUSAGE_SELF: i32 = 0;
    const RUSAGE_CHILDREN: i32 = -1;
    const RUSAGE_THREAD: i32 = 1;
    let mut ret = Rusage::default();
    match who {
        RUSAGE_SELF => {
            let (mut total_utime, mut totol_stime) = current_task().time_stat().user_system_time();
            current_task().with_thread_group(|tg| {
                for thread in tg.iter() {
                    let (utime, stime) = thread.time_stat().user_system_time();
                    total_utime += utime;
                    totol_stime += stime;
                }
            });

            ret.utime = total_utime.into();
            ret.stime = totol_stime.into();
            usage.write(current_task(), ret);
        }
        RUSAGE_CHILDREN => {
            unimplemented!()
        }
        RUSAGE_THREAD => {
            unimplemented!()
        }
        _ => return Err(SysError::EINVAL),
    }
    Ok(0)
}
