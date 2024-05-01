#![no_std]
#![no_main]

extern crate alloc;
use core::{cell::UnsafeCell, ptr, sync::atomic::AtomicPtr, task::Waker};

use hashbrown::HashMap;

pub(crate) type Tid = usize;
pub struct Futexes {
    pub map: HashMap<u32, UnsafeCell<HashMap<Tid, Waker>>>,
    pub robust_list: AtomicPtr<RobustListHead>,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct RobustListHead {
    pub list: usize,
    pub futex_offset: usize,
    pub list_op_pending: usize,
}

fn pop_waiter(waiters: &mut HashMap<Tid, Waker>) -> Option<(Tid, Waker)> {
    let mut key = None;
    if let Some((tid, _)) = waiters.iter().next() {
        key = Some(*tid);
    }
    if let Some(tid) = key {
        let waker = waiters.remove(&tid).unwrap();
        Some((tid, waker))
    } else {
        None
    }
}

impl Futexes {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            robust_list: AtomicPtr::new(ptr::null_mut()),
        }
    }

    pub fn add_waiter(&mut self, uaddr: u32, tid: Tid, waker: Waker) {
        if let Some(waiters) = self.map.get(&uaddr) {
            unsafe { &mut *waiters.get() }.insert(tid, waker);
        } else {
            let mut waiters = HashMap::new();
            waiters.insert(tid, waker);
            self.map.insert(uaddr, UnsafeCell::new(waiters));
        }
    }

    pub fn waiters(&self, uaddr: u32) -> Option<&mut HashMap<Tid, Waker>> {
        if let Some(waiters) = self.map.get(&uaddr) {
            Some(unsafe { &mut *waiters.get() })
        } else {
            None
        }
    }

    pub fn remove_waiter(&mut self, uaddr: u32, tid: Tid) {
        let waiters = self.waiters(uaddr).unwrap();
        waiters.remove(&tid).expect("no waiters of this tid");
    }

    pub fn wake(&mut self, uaddr: u32, n: u32) -> usize {
        let mut count = 0;
        if let Some(waiters) = self.waiters(uaddr) {
            while let Some((_, waiter)) = pop_waiter(waiters) {
                waiter.wake();
                count += 1;
                if count == n {
                    break;
                }
            }
        }
        count as usize
    }

    /// Wakes up a maximum of `n_wake` waiters that are waiting on the futex at
    /// `old_uaddr`.
    /// The remaining waiters with maxnum `n_rq`  are removed from the wait
    /// queue of `old_uaddr` and added to the wait queue of `new_uaddr`
    pub fn requeue_waiters(
        &mut self,
        old_uaddr: u32,
        new_uaddr: u32,
        n_wake: u32,
        n_rq: u32,
    ) -> usize {
        if old_uaddr == new_uaddr {
            return 0;
        }
        let new_waiters = match self.map.get(&new_uaddr) {
            None => {
                self.map.insert(new_uaddr, UnsafeCell::new(HashMap::new()));
                unsafe { &mut *self.map.get(&new_uaddr).unwrap().get() }
            }
            Some(new_waiters) => unsafe { &mut *new_waiters.get() },
        };
        let wake_count = self.wake(old_uaddr, n_wake);
        let Some(old_waiters) = self.waiters(old_uaddr) else {
            return wake_count;
        };
        let mut rq_count = 0;
        while let Some((tid, waker)) = pop_waiter(old_waiters) {
            new_waiters.insert(tid, waker);
            rq_count += 1;
            if rq_count == n_rq {
                break;
            }
        }
        rq_count as usize + wake_count
    }
}
