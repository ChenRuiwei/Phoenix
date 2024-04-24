use core::{cmp::Reverse, task::Waker, time::Duration};
extern crate alloc;
use alloc::collections::BinaryHeap;

use spin::Lazy;
use sync::mutex::SpinNoIrqLock;

pub struct Timer {
    pub expire: Duration,
    pub callback: Option<Waker>,
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.expire.cmp(&other.expire)
    }
}

impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.expire.partial_cmp(&other.expire)
    }
}

impl Eq for Timer {}

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool {
        self.expire == other.expire
    }
}

pub struct TimerManager {
    timers: SpinNoIrqLock<BinaryHeap<Reverse<Timer>>>,
}

impl TimerManager {
    fn new() -> Self {
        Self {
            timers: SpinNoIrqLock::new(BinaryHeap::new()),
        }
    }

    pub fn add_timer(&self, timer: Timer) {
        self.timers.lock().push(Reverse(timer));
    }

    pub fn check(&self, current: Duration) {
        let mut timers = self.timers.lock();
        if let Some(timer) = timers.peek() {
            if current >= timer.0.expire {
                log::info!(
                    "[Timer Manager] there is a timer expired, current:{:?}, expire:{:?}",
                    current,
                    timer.0.expire
                );
                let mut timer = timers.pop().unwrap().0;
                timer.callback.take().unwrap().wake();
            }
        }
    }
}

pub static TIMER_MANAGER: Lazy<TimerManager> = Lazy::new(TimerManager::new);
