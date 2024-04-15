use core::time::Duration;

use arch::time::get_time_duration;

///                                -user-          --user--
/// ---kernel---(switch)---kernel--      --kernel--        ------(switch)
///      switch_out    switch_in  ret   trap      ret    trap switch_out
// TODO: what about kernel interrupt
// HACK: use state machine implementation will be much clearer
pub struct TaskTimeStat {
    task_start: Duration,
    user_time: Duration,
    system_time: Duration,
    switch_in: Duration,
    switch_out: Duration,
    last_trap_ret: Duration,
    last_trap: Duration,
}

impl TaskTimeStat {
    pub fn new() -> Self {
        let start = get_time_duration();
        Self {
            task_start: start,
            user_time: Duration::ZERO,
            system_time: Duration::ZERO,
            switch_in: start,
            switch_out: start,
            last_trap_ret: Duration::ZERO,
            last_trap: Duration::ZERO,
        }
    }

    pub fn user_time(&self) -> Duration {
        self.user_time
    }

    pub fn system_time(&self) -> Duration {
        self.system_time
    }

    pub fn record_switch_in(&mut self) {
        self.switch_in = get_time_duration();
    }

    pub fn record_switch_out(&mut self) {
        self.switch_out = get_time_duration();
        self.system_time += if self.last_trap == Duration::ZERO {
            self.switch_out - self.switch_in
        } else {
            self.switch_out - self.last_trap
        };
    }

    pub fn record_trap(&mut self) {
        self.last_trap = get_time_duration();
        self.user_time += self.last_trap - self.last_trap_ret;
        self.last_trap_ret = Duration::ZERO;
    }

    pub fn record_trap_return(&mut self) {
        self.last_trap_ret = get_time_duration();
        self.system_time += if self.last_trap == Duration::ZERO {
            self.last_trap_ret - self.switch_in
        } else {
            self.last_trap_ret - self.last_trap
        };
    }
}
