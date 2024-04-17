#[derive(Clone, Copy)]
pub struct TimeSpec {
    tv_sec: usize,
    tv_nsec: usize,
}

impl TimeSpec {
    pub fn into_ms(&self) -> usize {
        self.tv_sec * 1_000 + self.tv_nsec / 1_000_000
    }

    pub fn from_ms(ms: usize) -> Self {
        Self {
            tv_sec: ms / 1000,
            tv_nsec: (ms % 1000) * 1000,
        }
    }
}
