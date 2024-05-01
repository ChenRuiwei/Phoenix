use systype::{SysError, SyscallResult};
use time::timeval::TimeVal;

use crate::{mm::UserWritePtr, processor::hart::current_task};

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
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
///   calling process that have terminated and been waited for. These statistics
///   will include the resources used by grandchildren, and further removed
///   descendants, if all of the intervening descendants waited on their
///   terminated children.
/// - RUSAGE_THREAD: Return resource usage statistics for the calling thread.
pub fn sys_getrusage(who: i32, usage: UserWritePtr<Rusage>) -> SyscallResult {
    let task = current_task();
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
            unimplemented!()
        }
        RUSAGE_THREAD => {
            unimplemented!()
        }
        _ => return Err(SysError::EINVAL),
    }
    Ok(0)
}
