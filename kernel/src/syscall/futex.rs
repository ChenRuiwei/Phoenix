use async_utils::suspend_now;
use systype::{SysError, SyscallResult};
use time::timespec::TimeSpec;

use super::Syscall;
use crate::{
    ipc::futex::{futex_manager, FutexHashKey, FutexWaiter, RobustListHead},
    mm::{FutexAddr, UserReadPtr, UserWritePtr},
};

impl Syscall<'_> {
    /// futex - fast user-space locking
    /// # Arguments
    /// - `uaddr`: points  to the futex word.  On all platforms, futexes are
    ///   four-byte integers that must be aligned on a four-byte boundary.
    /// - `futex_op`: The operation to perform on the futex. The argument
    ///   consists of two parts: a command that specifies the operation to be
    ///   performed, bitwise ORed with zero or more options that modify the
    ///   behaviour of the operation.
    /// - `val`: a value whose meaning and  purpose  depends on futex_op.
    /// - `timeout`: a pointer to a timespec structure that specifies a timeout
    ///   for the operation.
    /// - `uaddr2`: a pointer to a second futex word that is employed by the
    ///   operation.
    /// - `val3`: depends on the operation.
    pub async fn sys_futex(
        &self,
        uaddr: FutexAddr,
        futex_op: i32,
        val: u32,
        timeout: usize,
        uaddr2: usize,
        val3: u32,
    ) -> SyscallResult {
        /// Tells the kernel that the futex is process-private and not shared
        /// with another process. This OS doesn't support this flag
        const FUTEX_PRIVATE_FLAG: i32 = 0x80;
        // const FUTEX_CLOCK_REALTIME: i32 = 0x100;
        /// Tests that the value at the futex word pointed to by the address
        /// uaddr still contains the expected value val, and if so, then
        /// sleeps waiting for a FUTEX_WAKE operation on the futex word.
        const FUTEX_WAIT: i32 = 0;
        /// Wakes at most val of the waiters that are waiting (e.g., inside
        /// FUTEX_WAIT) on the futex word at the address uaddr.  Most commonly,
        /// val is specified as either 1 (wake up a single waiter) or
        /// INT_MAX (wake up all waiters). No guarantee is provided
        /// about which waiters are awoken
        const FUTEX_WAKE: i32 = 1;
        // TODO: need fild descriptor
        // const FUTEX_FD: i32 = 2;
        /// Performs the same task as FUTEX_CMP_REQUEUE (see
        /// below), except that no check is made using the value in val3. (The
        /// argument val3 is ignored.)
        const FUTEX_REQUEUE: i32 = 3;
        /// First checks whether the location uaddr still contains the value
        /// `val3`. If not, the operation fails with the error EAGAIN.
        /// Otherwise, the operation wakes up a maximum of `val` waiters
        /// that are waiting on the futex at `uaddr`. If there are more
        /// than `val` waiters, then the remaining waiters are removed
        /// from the wait queue of the source futex at `uaddr` and added
        /// to the wait queue  of  the  target futex at `uaddr2`.  The
        /// `val2` argument specifies an upper limit on the
        /// number of waiters that are requeued to the futex at `uaddr2`.
        const FUTEX_CMP_REQUEUE: i32 = 4;
        // const FUTEX_WAKE_OP: i32 = 5;
        // const FUTEX_WAIT_BITSET: i32 = 9;
        // const FUTEX_WAKE_BITSET: i32 = 10;
        let task = self.task;
        uaddr.check(&task)?;
        let futex_key = if futex_op & FUTEX_PRIVATE_FLAG != 0 {
            FutexHashKey::Private {
                mm: task.raw_mm_pointer(),
                virtaddr: uaddr.addr,
            }
        } else {
            let phyaddr = task.with_memory_space(|mm| mm.va2pa(uaddr.addr));
            FutexHashKey::Shared { phyaddr }
        };
        let futex_op = futex_op & !FUTEX_PRIVATE_FLAG;
        log::warn!("[sys_futex] uaddr:{:#x} key:{:?}", uaddr.raw(), futex_key);

        match futex_op {
            FUTEX_WAIT => {
                let res = uaddr.read();
                if res != val {
                    log::warn!(
                        "[futex_wait] value in {} addr is {res} but expect {val}",
                        uaddr.addr.0
                    );
                    return Err(SysError::EAGAIN);
                }
                futex_manager().add_waiter(
                    &futex_key,
                    FutexWaiter {
                        tid: task.tid(),
                        waker: task.waker().clone().unwrap(),
                    },
                );
                task.set_interruptable();
                task.set_wake_up_signal(!*task.sig_mask_ref());
                if timeout == 0 {
                    suspend_now().await;
                } else {
                    let timeout = UserReadPtr::<TimeSpec>::from(timeout as usize).read(&task)?;
                    log::warn!("[futex_wait] waiting for {:?}", timeout);
                    if !timeout.is_valid() {
                        return Err(SysError::EINVAL);
                    }
                    let rem = task.suspend_timeout(timeout.into()).await;
                    if rem.is_zero() {
                        futex_manager().remove_waiter(&futex_key, task.tid());
                    }
                }
                log::warn!("[sys_futex] I was woken");
                task.set_running();
                Ok(0)
            }
            FUTEX_WAKE => {
                let n_wake = futex_manager().wake(&futex_key, val)?;
                return Ok(n_wake);
            }
            FUTEX_CMP_REQUEUE => {
                if uaddr.read() as u32 != val3 {
                    return Err(SysError::EAGAIN);
                }
                let n_wake = futex_manager().wake(&futex_key, val)?;
                let new_key = if futex_op & FUTEX_PRIVATE_FLAG != 0 {
                    FutexHashKey::Private {
                        mm: task.raw_mm_pointer(),
                        virtaddr: uaddr2.into(),
                    }
                } else {
                    let phyaddr = task.with_memory_space(|mm| mm.va2pa(uaddr2.into()));
                    FutexHashKey::Shared { phyaddr }
                };
                futex_manager().requeue_waiters(futex_key, new_key, timeout)?;
                Ok(n_wake)
            }
            FUTEX_REQUEUE => {
                let n_wake = futex_manager().wake(&futex_key, val)?;
                let new_key = if futex_op & FUTEX_PRIVATE_FLAG != 0 {
                    FutexHashKey::Private {
                        mm: task.raw_mm_pointer(),
                        virtaddr: uaddr2.into(),
                    }
                } else {
                    let phyaddr = task.with_memory_space(|mm| mm.va2pa(uaddr2.into()));
                    FutexHashKey::Shared { phyaddr }
                };
                futex_manager().requeue_waiters(futex_key, new_key, timeout)?;
                Ok(n_wake)
            }
            _ => panic!("unimplemented futexop {:?}", futex_op),
        }
    }

    /// actually this syscall has no actual effect
    pub fn sys_get_robust_list(
        &self,
        pid: i32,
        robust_list_head: UserWritePtr<RobustListHead>,
        len_ptr: UserWritePtr<usize>,
    ) -> SyscallResult {
        // let Some(task) = TASK_MANAGER.get(pid as usize) else {
        //     return Err(SysError::ESRCH);
        // };
        // if !task.is_leader() {
        //     return Err(SysError::ESRCH);
        // }
        // // UserReadPtr::<RobustListHead>::from(value)
        // len_ptr.write(&task, mem::size_of::<RobustListHead>())?;
        // robust_list_head.write(&task, unsafe {
        //     *task.with_futexes(|futexes| futexes.robust_list.load(Ordering::SeqCst))
        // })?;
        Ok(0)
    }

    /// actually this syscall has no actual effect
    pub fn sys_set_robust_list(
        &self,
        robust_list_head: UserReadPtr<RobustListHead>,
        len: usize,
    ) -> SyscallResult {
        // let task = self.task;
        // if len != mem::size_of::<RobustListHead>() {
        //     return Err(SysError::EINVAL);
        // }
        // let mut head = robust_list_head.into_ref(&task)?;
        // task.with_mut_futexes(|futexes| {
        //     futexes.robust_list.store(head.ptr_mut(), Ordering::SeqCst)
        // });
        Ok(0)
    }
}
