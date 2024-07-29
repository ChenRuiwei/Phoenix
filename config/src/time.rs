use core::time::Duration;

pub const INTERRUPTS_PER_SECOND: usize = 50;
pub const NANOSECONDS_PER_SECOND: usize = 1_000_000_000;
pub const TIME_SLICE_DUATION: Duration =
    Duration::new(0, (NANOSECONDS_PER_SECOND / INTERRUPTS_PER_SECOND) as u32);
