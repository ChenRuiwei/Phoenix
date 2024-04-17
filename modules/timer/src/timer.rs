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
    timers: SpinNoIrqLock<BinaryHeap<Reverse<Timer>>>
}

impl TimerManager {
    fn new() -> Self {
        Self {
            timers: SpinNoIrqLock::new(BinaryHeap::new())
        }
    }

    pub fn add_timer(&self, timer: Timer){
        self.timers.lock().push(Reverse(timer));
    }
}

pub static TIMER_MANAGER: Lazy<TimerManager> = Lazy::new(TimerManager::new);