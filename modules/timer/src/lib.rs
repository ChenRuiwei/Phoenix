#![no_std]
#![no_main]
use core::{cmp::Reverse, task::Waker, time::Duration};
extern crate alloc;
use alloc::{boxed::Box, collections::BinaryHeap};

use arch::time::get_time_duration;
use spin::Lazy;
use sync::mutex::SpinNoIrqLock;

pub mod timelimited_task;

pub type TimerCallback = fn(Box<dyn TimerData>);

/// Timer data trait to generalize data storage
pub trait TimerData: Send + Sync {
    fn callback(self: Box<Self>);
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
    /// This function allows the timer to perform specific operations upon
    /// expiration, such as handling timeout events, etc. The parameter of the
    /// function is an unsigned long value, usually passing the value of the
    /// `data` member
    pub function: TimerCallback,
    /// The parameters passed to the callback function. It can be any data that
    /// needs to be used in callback functions, such as structure pointers, flag
    /// values, etc. This member enables callback functions to access specific
    /// contextual data when called, thereby performing more complex operations.
    pub data: Box<dyn TimerData>,
}
fn simple_callback(data: Box<dyn TimerData>) {
    data.callback()
}
impl Timer {
    pub fn new(expire: Duration, function: TimerCallback, data: Box<dyn TimerData>) -> Self {
        Self {
            expire,
            function,
            data,
        }
    }
    pub fn new_waker_timer(expire: Duration, waker: Waker) -> Self {
        struct WakerData {
            waker: Waker,
        }
        impl TimerData for WakerData {
            fn callback(self: Box<Self>) {
                self.waker.wake();
            }
        }

        Self {
            expire,
            function: simple_callback,
            data: Box::new(WakerData { waker }),
        }
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
                let mut timer = timers.pop().unwrap().0;
                (timer.function)(timer.data);
            } else {
                break;
            }
        }
    }
}

pub static TIMER_MANAGER: Lazy<TimerManager> = Lazy::new(TimerManager::new);
