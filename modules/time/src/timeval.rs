use core::time::Duration;

#[derive(Debug, Clone, Copy, Default)]
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

impl TimeVal {
    pub fn from_usec(usec: usize) -> Self {
        Self {
            tv_sec: usec / 1_000_000,
            tv_usec: usec % 1_000_000,
        }
    }
}
