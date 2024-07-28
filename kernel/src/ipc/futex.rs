use alloc::vec::Vec;
use core::{cmp::min, hash::Hash, ops::DerefMut, task::Waker};

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

pub static FUTEX_MANAGER: Lazy<SpinNoIrqLock<FutexManager>> =
    Lazy::new(|| SpinNoIrqLock::new(FutexManager::new()));

pub fn futex_manager() -> impl DerefMut<Target = FutexManager> {
    FUTEX_MANAGER.lock()
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Copy, Clone)]
pub enum FutexHashKey {
    Shared { paddr: PhysAddr },
    Private { mm: usize, vaddr: VirtAddr },
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
pub struct FutexManager(HashMap<FutexHashKey, Vec<FutexWaiter>>);

impl FutexManager {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn add_waiter(&mut self, key: &FutexHashKey, waiter: FutexWaiter) {
        log::info!("[futex::add_waiter] {:?} in {:?} ", waiter, key);
        if let Some(waiters) = self.0.get_mut(key) {
            waiters.push(waiter);
        } else {
            let mut waiters = Vec::new();
            waiters.push(waiter);
            self.0.insert(*key, waiters);
        }
    }

    /// 用于移除任务，任务可能是过期了，也可能是被信号中断了
    pub fn remove_waiter(&mut self, key: &FutexHashKey, tid: Tid) {
        if let Some(waiters) = self.0.get_mut(key) {
            for i in 0..waiters.len() {
                if waiters[i].tid == tid {
                    waiters.swap_remove(i);
                    break;
                }
            }
        }
    }

    pub fn wake(&mut self, key: &FutexHashKey, n: u32) -> SyscallResult {
        if let Some(waiters) = self.0.get_mut(key) {
            let n = min(n as usize, waiters.len());
            for _ in 0..n {
                let waiter = waiters.pop().unwrap();
                log::info!("[futex_wake] {:?} has been woken", waiter);
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
            log::debug!("can not find key {key:?}");
            Err(SysError::EINVAL)
        }
    }

    pub fn requeue_waiters(
        &mut self,
        old: FutexHashKey,
        new: FutexHashKey,
        n_req: usize,
    ) -> SyscallResult {
        let mut old_waiters = self.0.remove(&old).ok_or_else(|| {
            log::info!("[futex] no waiters in key {:?}", old);
            SysError::EINVAL
        })?;
        let n = min(n_req as usize, old_waiters.len());
        if let Some(new_waiters) = self.0.get_mut(&new) {
            for _ in 0..n {
                new_waiters.push(old_waiters.pop().unwrap());
            }
        } else {
            let mut new_waiters = Vec::with_capacity(n);
            for _ in 0..n {
                new_waiters.push(old_waiters.pop().unwrap());
            }
            self.0.insert(new, new_waiters);
        }

        if !old_waiters.is_empty() {
            self.0.insert(old, old_waiters);
        }

        Ok(n)
    }
}

bitflags! {
    #[repr(C)]
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
    pub struct FutexOp: i32 {
        /// Tests that the value at the futex word pointed to by the address
        /// uaddr still contains the expected value val, and if so, then
        /// sleeps waiting for a FUTEX_WAKE operation on the futex word.
        const Wait = 0;
        /// Wakes at most val of the waiters that are waiting (e.g., inside
        /// FUTEX_WAIT) on the futex word at the address uaddr.  Most commonly,
        /// val is specified as either 1 (wake up a single waiter) or
        /// INT_MAX (wake up all waiters). No guarantee is provided
        /// about which waiters are awoken
        const Wake = 1;
        const Fd = 2;
        // const FUTEX_FD: i32 = 2;
        /// Performs the same task as FUTEX_CMP_REQUEUE (see
        /// below), except that no check is made using the value in val3. (The
        /// argument val3 is ignored.)
        const Requeue = 3;
        /// First checks whether the location uaddr still contains the value
        /// `val3`. If not, the operation fails with the error EAGAIN.
        /// Otherwise, the operation wakes up a maximum of `val` waiters
        /// that are waiting on the futex at `uaddr`. If there are more
        /// than `val` waiters, then the remaining waiters are removed
        /// from the wait queue of the source futex at `uaddr` and added
        /// to the wait queue  of  the  target futex at `uaddr2`.  The
        /// `val2` argument specifies an upper limit on the
        /// number of waiters that are requeued to the futex at `uaddr2`.
        const CmpRequeue = 4;
        const WakeOp = 5;
        const LockPi = 6;
        const UnlockPi = 7;
        const TrylockPi = 8;
        const WaitBitset = 9;
        const WakeBitset = 10;
        const WaitRequeuePi = 11;
        /// Tells the kernel that the futex is process-private and not shared
        /// with another process.
        const Private = 128;
    }
}
