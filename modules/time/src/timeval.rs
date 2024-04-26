use core::time::Duration;

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
/// Describes times in seconds and microseconds.
pub struct TimeVal {
    /// second
    tv_sec: usize,
    /// millisecond
    tv_usec: usize,
}

impl From<Duration> for TimeVal {
    fn from(duration: Duration) -> Self {
        Self {
            tv_sec: duration.as_secs() as usize,
            tv_usec: duration.subsec_micros() as usize,
        }
    }
}

impl Into<Duration> for TimeVal {
    fn into(self) -> Duration {
        Duration::new(self.tv_sec as u64, (self.tv_usec * 1000) as u32)
    }
}

impl TimeVal {
    pub fn from_usec(usec: usize) -> Self {
        Self {
            tv_sec: usec / 1_000_000,
            tv_usec: usec % 1_000_000,
        }
    }

    pub fn into_usec(&self) -> usize {
        self.tv_sec * 1_000_000 + self.tv_usec
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct ITimerVal {
    /// Interval for periodic timer
    it_interval: TimeVal,
    /// Time until next expiration
    it_value: TimeVal,
}

impl ITimerVal {
    pub fn interval_duration(&self) -> Duration {
        self.it_interval.into()
    }

    pub fn value_duration(&self) -> Duration {
        self.it_value.into()
    }
}
