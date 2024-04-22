use crate::stat::TaskTimeStat;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TMS {
    /// User CPU time used by caller
    tms_utime: usize,
    /// System CPU time used by caller
    tms_stime: usize,
    /// User CPU time of all (waited for)
    /// children(已终止的子进程累积的用户态时间)
    tms_cutime: usize,
    /// System CPU time of all (waited for)
    /// children(已终止的子进程累积的核心态时间)
    tms_cstime: usize,
}

impl TMS {
    pub fn from_task_time_stat(tts: &TaskTimeStat) -> Self {
        let (utime, stime) = tts.user_system_time();
        let (cutime, cstime) = tts.child_user_system_time();
        Self {
            tms_utime: utime.as_micros() as usize,
            tms_stime: stime.as_micros() as usize,
            tms_cutime: cutime.as_micros() as usize,
            tms_cstime: cstime.as_micros() as usize,
        }
    }
}
