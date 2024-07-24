use core::time::Duration;

use arch::time::get_time_duration;
use config::time::TIME_SLICE_DUATION;

///                                -user-          --user--
/// ---kernel---(switch)---kernel--      --kernel--        ------(switch)
///      switch_out    switch_in  ret   trap      ret    trap switch_out
// TODO: what about kernel interrupt
pub struct TaskTimeStat {
    user_time: Duration,
    system_time: Duration,

    // task_start: Duration,
    system_time_start: Duration,
    user_time_start: Duration,
    schedule_time_start: Duration,

    child_user_time: Duration,
    child_system_stime: Duration,
}

impl TaskTimeStat {
    pub const fn new() -> Self {
        // let start = get_time_duration();
        Self {
            // task_start: start,
            user_time: Duration::ZERO,
            system_time: Duration::ZERO,
            child_user_time: Duration::ZERO,
            child_system_stime: Duration::ZERO,
            system_time_start: Duration::ZERO,
            user_time_start: Duration::ZERO,
            schedule_time_start: Duration::ZERO,
        }
    }

    /// return the cutime and cstime
    pub fn user_system_time(&self) -> (Duration, Duration) {
        (self.user_time, self.system_time)
    }

    pub fn child_user_system_time(&self) -> (Duration, Duration) {
        (self.child_user_time, self.child_system_stime)
    }

    #[inline]
    pub fn user_time(&self) -> Duration {
        self.user_time
    }

    #[inline]
    pub fn sys_time(&self) -> Duration {
        self.system_time
    }

    pub fn cpu_time(&self) -> Duration {
        self.user_time + self.system_time
    }

    pub fn update_child_time(&mut self, (utime, stime): (Duration, Duration)) {
        self.child_user_time += utime;
        self.child_system_stime += stime;
    }

    pub fn record_switch_in(&mut self) {
        let current_time = get_time_duration();

        self.system_time_start = current_time;
        self.schedule_time_start = current_time;
    }

    pub fn record_switch_out(&mut self) {
        let stime_slice = get_time_duration() - self.system_time_start;
        self.system_time += stime_slice;
    }

    pub fn record_trap(&mut self) {
        let current_time = get_time_duration();

        self.system_time_start = current_time;

        let utime_slice = current_time - self.user_time_start;
        self.user_time += utime_slice;
    }

    pub fn record_trap_return(&mut self) {
        let current_time = get_time_duration();

        let stime_slice = current_time - self.user_time_start;
        self.system_time += stime_slice;

        self.user_time_start = current_time;
    }

    pub fn need_schedule(&self) -> bool {
        get_time_duration() - self.schedule_time_start >= TIME_SLICE_DUATION
    }
}
