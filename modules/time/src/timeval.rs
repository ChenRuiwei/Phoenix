#[derive(Clone, Copy)]
pub struct TimeVal {
    /// second
    tv_sec: usize,
    /// millisecond
    tv_usec: usize,
}

impl TimeVal {
    pub fn from_usec(usec: usize) -> Self {
        Self {
            tv_sec: usec / 1_000_000,
            tv_usec: usec % 1_000_000,
        }
    }
}