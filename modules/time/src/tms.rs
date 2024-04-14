use crate::stat::TaskTimeStat;

#[derive(Clone, Copy)]
pub struct TMS {
    /// User CPU time used by caller
    tms_utime: usize,
    /// System CPU time used by caller
    tms_stime: usize,
    /// User CPU time of all (waited for) children(已终止的子进程累积的用户态时间)
    tms_cutime: usize,
    /// System CPU time of all (waited for) children(已终止的子进程累积的核心态时间)
    tms_cstime: usize,
}

impl TMS {
    pub fn from_task_time_stat(tts: &TaskTimeStat) -> Self {
        // FIXME: tms_cutime and tms_cstime should be set in sys_wait4
        Self {
            tms_utime: tts.user_time.as_micros() as usize,
            tms_stime: tts.system_time.as_micros() as usize,
            tms_cutime: tts.user_time.as_micros() as usize,
            tms_cstime: tts.system_time.as_micros() as usize,
        }
    }
}