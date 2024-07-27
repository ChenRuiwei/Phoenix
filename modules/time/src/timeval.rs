use core::{fmt, time::Duration};

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
/// Describes times in seconds and microseconds.
pub struct TimeVal {
    /// second
    tv_sec: usize,
    /// microsecond
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
        Duration::new(self.tv_sec as u64, (self.tv_usec * 1_000) as u32)
    }
}

impl TimeVal {
    pub const ZERO: Self = Self {
        tv_sec: 0,
        tv_usec: 0,
    };
    pub fn from_usec(usec: usize) -> Self {
        Self {
            tv_sec: usec / 1_000_000,
            tv_usec: usec % 1_000_000,
        }
    }

    pub fn into_usec(&self) -> usize {
        self.tv_sec * 1_000_000 + self.tv_usec
    }

    pub fn is_valid(&self) -> bool {
        self.tv_usec < 1_000_000
    }

    pub fn is_zero(&self) -> bool {
        self.tv_sec == 0 && self.tv_usec == 0
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct ITimerVal {
    /// Interval for periodic timer
    pub it_interval: TimeVal,
    /// Time until next expiration
    pub it_value: TimeVal,
}

impl ITimerVal {
    pub const ZERO: Self = Self {
        it_interval: TimeVal::ZERO,
        it_value: TimeVal::ZERO,
    };

    pub fn is_valid(&self) -> bool {
        self.it_interval.is_valid() && self.it_value.is_valid()
    }

    pub fn is_enabled(&self) -> bool {
        !(self.it_interval.is_zero() && self.it_value.is_zero())
    }
}

impl fmt::Display for TimeVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let seconds = self.tv_sec;
        let microseconds = self.tv_usec as f64 / 1_000_000.0;
        write!(f, "{:.6}s", seconds as f64 + microseconds)
    }
}

impl fmt::Display for ITimerVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "interval: {}, value: {}",
            self.it_interval, self.it_value
        )
    }
}
