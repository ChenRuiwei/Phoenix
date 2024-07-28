#![no_std]
#![no_main]
use core::{cmp::Reverse, task::Waker, time::Duration};
extern crate alloc;
use alloc::{boxed::Box, collections::BinaryHeap};


use spin::Lazy;
use sync::mutex::SpinNoIrqLock;

pub mod timelimited_task;

/// Timer data trait to generalize data storage
pub trait TimerEvent: Send + Sync {
    /// This function allows the timer to perform specific operations upon
    /// expiration, such as handling timeout events, etc.
    fn callback(self: Box<Self>) -> Option<Timer>;
}

/// 定时器
pub struct Timer {
    /// The expiration time of the timer.
    ///
    /// The kernel periodically checks the system's tick count and compares the
    /// expires of each timer in the linked list. If the current time exceeds or
    /// equals the expires value, the timer is considered expired and triggers
    /// the corresponding processing function
    pub expire: Duration,
    /// The parameters passed to the callback function. It can be any data that
    /// needs to be used in callback functions, such as structure pointers, flag
    /// values, etc. This member enables callback functions to access specific
    /// contextual data when called, thereby performing more complex operations.
    pub data: Box<dyn TimerEvent>,
}

impl Timer {
    pub fn new(expire: Duration, data: Box<dyn TimerEvent>) -> Self {
        Self { expire, data }
    }

    pub fn new_waker_timer(expire: Duration, waker: Waker) -> Self {
        struct WakerData {
            waker: Waker,
        }
        impl TimerEvent for WakerData {
            fn callback(self: Box<Self>) -> Option<Timer> {
                self.waker.wake();
                None
            }
        }

        Self {
            expire,
            data: Box::new(WakerData { waker }),
        }
    }

    fn callback(self) -> Option<Timer> {
        self.data.callback()
    }
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.expire.cmp(&other.expire)
    }
}

impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
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
        log::debug!("add new timer, next expiration {:?}", timer.expire);
        self.timers.lock().push(Reverse(timer));
    }

    pub fn check(&self, current: Duration) {
        let mut timers = self.timers.lock();
        while let Some(timer) = timers.peek() {
            if current >= timer.0.expire {
                log::info!(
                    "[Timer Manager] there is a timer expired, current:{:?}, expire:{:?}",
                    current,
                    timer.0.expire
                );
                let timer = timers.pop().unwrap().0;
                if let Some(new_timer) = timer.callback() {
                    timers.push(Reverse(new_timer));
                }
            } else {
                break;
            }
        }
    }
}

pub static TIMER_MANAGER: Lazy<TimerManager> = Lazy::new(TimerManager::new);
