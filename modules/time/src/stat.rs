use core::time::Duration;
///                                -user-          --user--
/// ---kernel---(switch)---kernel--      --kernel--        ------(switch)
///      switch_out    switch_in  ret   trap      ret    trap switch_out
pub struct TaskTimeStat {
    pub task_start: Duration,
    pub user_time: Duration,
    pub system_time: Duration,
    pub switch_in: Duration,
    pub switch_out: Duration,
    pub last_trap_ret: Duration,
    pub last_trap: Duration,
}

impl TaskTimeStat {
    pub fn new(start: Duration) -> Self {
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

    pub fn record_switch_in_time(&mut self, cur_time: Duration) {
        self.switch_in = cur_time;
    }

    pub fn record_switch_out_time(&mut self, cur_time: Duration) {
        self.switch_out = cur_time;
        self.system_time += if self.last_trap == Duration::ZERO {
            self.switch_out - self.switch_in
        } else {
            self.switch_out - self.last_trap
        };
    }

    pub fn record_trap_time(&mut self, cur_time: Duration) {
        self.last_trap = cur_time;
        self.user_time += self.last_trap - self.last_trap_ret;
        self.last_trap_ret = Duration::ZERO;
    }

    pub fn record_trap_return_time(&mut self, cur_time: Duration) {
        self.last_trap_ret = cur_time;
        self.system_time += if self.last_trap == Duration::ZERO {
            self.last_trap_ret - self.switch_in
        } else {
            self.last_trap_ret - self.last_trap
        };
    }
}
