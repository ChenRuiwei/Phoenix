use core::time::Duration;

use time::timeval::TimeVal;

use super::Task;

impl Task {
    pub fn get_process_ustime(&self) -> (Duration, Duration) {
        self.with_thread_group(|tg| -> (Duration, Duration) {
            tg.iter()
                .map(|thread| thread.time_stat().user_system_time())
                .reduce(|(acc_utime, acc_stime), (utime, stime)| {
                    (acc_utime + utime, acc_stime + stime)
                })
                .unwrap()
        })
    }

    pub fn get_process_utime(&self) -> Duration {
        self.with_thread_group(|tg| -> Duration {
            tg.iter()
                .map(|thread| thread.time_stat().user_time())
                .reduce(|acc_utime, utime| acc_utime + utime)
                .unwrap()
        })
    }

    pub fn get_process_cputime(&self) -> Duration {
        self.with_thread_group(|tg| -> Duration {
            tg.iter()
                .map(|thread| thread.time_stat().cpu_time())
                .reduce(|acc, cputime| acc + cputime)
                .unwrap()
        })
    }
}

bitflags! {
    #[derive(Clone, Copy)]
    #[repr(C)]
    pub struct CpuMask: usize {
        const CPU0 = 0b00000001;
        const CPU1 = 0b00000010;
        const CPU2 = 0b00000100;
        const CPU3 = 0b00001000;
        const CPU4 = 0b00010000;
        const CPU5 = 0b00100000;
        const CPU6 = 0b01000000;
        const CPU7 = 0b10000000;
        const CPU_ALL = 0b11111111;
    }
}

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

pub const RLIM_INFINITY: usize = usize::MAX;

/// Resource Limit
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RLimit {
    /// Soft limit: the kernel enforces for the corresponding resource
    pub rlim_cur: usize,
    /// Hard limit (ceiling for rlim_cur)
    pub rlim_max: usize,
}

impl RLimit {
    pub fn new(rlim_cur: usize) -> Self {
        Self {
            rlim_cur,
            rlim_max: RLIM_INFINITY,
        }
    }
}
