use alloc::vec::Vec;
use core::{cell::SyncUnsafeCell, cmp::min, hash::Hash, task::Waker};

use hashbrown::HashMap;
use memory::{PhysAddr, VirtAddr};
use spin::Lazy;
use sync::mutex::SpinNoIrqLock;
use systype::{SysError, SyscallResult};
type Tid = usize;

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct RobustListHead {
    pub list: usize,
    pub futex_offset: usize,
    pub list_op_pending: usize,
}

pub static FUTEX_MANAGER: Lazy<SyncUnsafeCell<FutexManager>> =
    Lazy::new(|| SyncUnsafeCell::new(FutexManager::new()));

pub fn futex_manager() -> &'static mut FutexManager {
    unsafe { &mut *FUTEX_MANAGER.get() }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Copy, Clone)]
pub enum FutexHashKey {
    Shared { phyaddr: PhysAddr },
    Private { mm: usize, virtaddr: VirtAddr },
}

#[derive(Debug)]
pub struct FutexWaiter {
    pub tid: Tid,
    pub waker: Waker,
}

impl FutexWaiter {
    pub fn wake(self) {
        self.waker.wake();
    }
}

/// `futex`: 一个32位的值，又称为`futex word`，将其地址传递给futex()系统调用
pub struct FutexManager(HashMap<FutexHashKey, SpinNoIrqLock<Vec<FutexWaiter>>>);

impl FutexManager {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    pub fn add_waiter(&mut self, key: &FutexHashKey, waiter: FutexWaiter) {
        log::warn!("[futex] add waiter {:?} in {:?} ", waiter, key);
        if let Some(waiters) = self.0.get(key) {
            waiters.lock().push(waiter);
        } else {
            let mut waiters = Vec::new();
            waiters.push(waiter);
            self.0.insert(*key, SpinNoIrqLock::new(waiters));
        }
    }

    /// 用于移除过期任务
    pub fn remove_waiter(&mut self, key: &FutexHashKey, tid: Tid) {
        if let Some(waiters) = self.0.get(key) {
            let mut waiters = waiters.lock();
            for i in 0..waiters.len() {
                if waiters[i].tid == tid {
                    waiters.swap_remove(i);
                    break;
                }
            }
        }
    }

    pub fn wake(&mut self, key: &FutexHashKey, n: u32) -> SyscallResult {
        if let Some(waiters) = self.0.get(key) {
            let mut waiters = waiters.lock();
            let n = min(n as usize, waiters.len());
            for _ in 0..n {
                let waiter = waiters.pop().unwrap();
                log::warn!("[futex_wake] waiter {:?} has been woken", waiter);
                waiter.wake();
            }
            drop(waiters);
            log::info!(
                "[futex_wake] wake {} waiters in key {:?}, expect to wake {} waiters",
                n,
                key,
                n,
            );
            Ok(n)
        } else {
            Err(SysError::EINVAL)
        }
    }

    pub fn requeue_waiters(
        &mut self,
        old: FutexHashKey,
        new: FutexHashKey,
        n_req: usize,
    ) -> SyscallResult {
        let mut old_waiters = self
            .0
            .get(&old)
            .ok_or_else(|| {
                log::warn!("[futex] no waiters in key {:?}", old);
                SysError::EINVAL
            })?
            .lock();
        let n = min(n_req as usize, old_waiters.len());
        if let Some(new_waiters) = self.0.get(&new) {
            let mut new_waiters = new_waiters.lock();
            for _ in 0..n {
                new_waiters.push(old_waiters.pop().unwrap());
            }
        } else {
            let mut new_waiters = Vec::with_capacity(n);
            for _ in 0..n {
                new_waiters.push(old_waiters.pop().unwrap());
            }
            drop(old_waiters);
            self.0.insert(new, SpinNoIrqLock::new(new_waiters));
        }
        Ok(n)
    }
}
