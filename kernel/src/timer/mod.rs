//! RISC-V timer-related functionality

pub mod ffi;
pub mod io_multiplex;
mod poll_queue;
pub mod timed_task;
pub mod timeout_task;
use alloc::collections::{BTreeMap, BinaryHeap};
use core::{cmp::Reverse, task::Waker, time::Duration};

use config::board::CLOCK_FREQ;
use driver::set_timer;
use log::info;
pub use poll_queue::POLL_QUEUE;
use riscv::register::time;
use sync::mutex::SpinNoIrqLock;

const TICKS_PER_SEC: usize = 100;
const MSEC_PER_SEC: usize = 1000;
const USEC_PER_SEC: usize = 1000000;

/// for clock_gettime
pub const CLOCK_REALTIME: usize = 0;
pub const CLOCK_MONOTONIC: usize = 1;
pub const CLOCK_PROCESS_CPUTIME_ID: usize = 2;

/// for utimensat
pub const UTIME_NOW: usize = 1073741823;
pub const UTIME_OMIT: usize = 1073741822;

/// for clock_nanosleep
pub const TIMER_ABSTIME: usize = 1;

/// get current time
fn get_time() -> usize {
    time::read()
}
/// get current time in milliseconds
pub fn current_time_ms() -> usize {
    time::read() / (CLOCK_FREQ / MSEC_PER_SEC)
}
/// get current time in microseconds
pub fn current_time_us() -> usize {
    time::read() / (CLOCK_FREQ / USEC_PER_SEC)
}
/// get current time in `Duration`
pub fn current_time_duration() -> Duration {
    Duration::from_micros(current_time_us() as u64)
}

/// set the next timer interrupt
pub fn set_next_trigger() {
    let next_trigger = get_time() + CLOCK_FREQ / TICKS_PER_SEC;
    // debug!("next trigger {}", next_trigger);
    set_timer(next_trigger);
}

/// clock stores the deviation: arg time - dev time(current_time)
pub struct ClockManager(pub BTreeMap<usize, Duration>);

/// Clock manager that used for looking for a given process
pub static CLOCK_MANAGER: SpinNoIrqLock<ClockManager> =
    SpinNoIrqLock::new(ClockManager(BTreeMap::new()));

pub fn init() {
    TIMER_QUEUE.init();

    CLOCK_MANAGER
        .lock()
        .0
        .insert(CLOCK_MONOTONIC, Duration::ZERO);

    CLOCK_MANAGER
        .lock()
        .0
        .insert(CLOCK_REALTIME, Duration::ZERO);

    poll_queue::init();
    info!("init clock manager success");
}

static TIMER_QUEUE: TimerQueue = TimerQueue::new();

/// Hold timers sorted by expired_time from earliest to latest
struct TimerQueue {
    timers: SpinNoIrqLock<Option<BinaryHeap<Reverse<Timer>>>>,
}

impl TimerQueue {
    const fn new() -> Self {
        Self {
            timers: SpinNoIrqLock::new(None),
        }
    }
    fn init(&self) {
        *self.timers.lock() = Some(BinaryHeap::new());
    }
    fn add_timer(&self, timer: Timer) {
        self.timers.lock().as_mut().unwrap().push(Reverse(timer))
    }
}

struct Timer {
    expired_time: Duration,
    waker: Option<Waker>,
}

impl Timer {
    fn is_expired(&self, current_time: Duration) -> bool {
        current_time >= self.expired_time
    }
}

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool {
        self.expired_time == other.expired_time
    }
}

impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Timer {}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.expired_time.cmp(&other.expired_time)
    }
}

pub fn handle_timeout_events() {
    let current_time = current_time_duration();
    let mut inner = TIMER_QUEUE.timers.lock();
    let timers = inner.as_mut().unwrap();
    // TODO: should we use SleepLock instead of SpinLock? It seems that the locking
    // time may be a little long.
    while let Some(Reverse(timer)) = timers.peek()
        && timer.is_expired(current_time)
    {
        let mut timer = timers.pop().unwrap().0;
        log::trace!(
            "[handle_timeout_events] find a timeout timer, current ts: {:?}, expired ts: {:?}",
            current_time,
            timer.expired_time
        );
        timer.waker.take().unwrap().wake();
    }
}
