use core::{
    future::Future,
    mem,
    pin::Pin,
    sync::atomic::Ordering,
    task::{Context, Poll},
};

use async_utils::yield_now;
use futex::{Futexes, RobustListHead};
use systype::{SysError, SyscallResult};
use time::timespec::TimeSpec;
use timer::timelimited_task::TimeLimitedTaskFuture;

use crate::{
    mm::{FutexWord, UserReadPtr, UserWritePtr},
    processor::hart::current_task,
    task::TASK_MANAGER,
};

/// futex - fast user-space locking
/// # Arguments
/// - `uaddr`: points  to the futex word.  On all platforms, futexes are
///   four-byte integers that must be aligned on a four-byte boundary.
/// - `futex_op`: The operation to perform on the futex. The argument consists
///   of two parts: a command that specifies the operation to be performed,
///   bitwise ORed with zero or more options that modify the behaviour of the
///   operation.
/// - `val`: a value whose meaning and  purpose  depends on futex_op.
/// - `timeout`: a pointer to a timespec structure that specifies a timeout for
///   the operation.
/// - `uaddr2`: a pointer to a second futex word that is employed by the
///   operation.
/// - `val3`: depends on the operation.
pub async fn sys_futex(
    uaddr: FutexWord,
    futex_op: i32,
    val: u32,
    timeout: u32,
    uaddr2: u32,
    val3: u32,
) -> SyscallResult {
    /// Tells the kernel that the futex is process-private and not shared with
    /// another process. This OS doesn't support this flag
    const FUTEX_PRIVATE_FLAG: i32 = 0x80;
    const FUTEX_CLOCK_REALTIME: i32 = 0x100;
    /// Tests that the value at the futex word pointed to by the address uaddr
    /// still contains the expected value val, and if so, then sleeps
    /// waiting for a FUTEX_WAKE operation on the futex word.
    const FUTEX_WAIT: i32 = 0;
    /// Wakes at most val of the waiters that are waiting (e.g., inside
    /// FUTEX_WAIT) on the futex word at the address uaddr.  Most commonly, val
    /// is specified as either 1 (wake up a single waiter) or INT_MAX (wake up
    /// all waiters). No guarantee is provided about which waiters are
    /// awoken
    const FUTEX_WAKE: i32 = 1;
    // TODO: need fild descriptor
    const FUTEX_FD: i32 = 2;
    /// Performs the same task as FUTEX_CMP_REQUEUE (see
    /// below), except that no check is made using the value in val3. (The
    /// argument val3 is ignored.)
    const FUTEX_REQUEUE: i32 = 3;
    /// First checks whether the location uaddr still contains the value `val3`.
    /// If not, the operation fails with the error EAGAIN. Otherwise, the
    /// operation wakes up a maximum of `val` waiters that are waiting on the
    /// futex at `uaddr`. If there are more than `val` waiters, then the
    /// remaining waiters are removed from the wait queue of the source
    /// futex at `uaddr` and added to the wait queue  of  the  target futex
    /// at `uaddr2`.  The `val2` argument specifies an upper limit on the
    /// number of waiters that are requeued to the futex at `uaddr2`.
    const FUTEX_CMP_REQUEUE: i32 = 4;
    // const FUTEX_WAKE_OP: i32 = 5;
    // const FUTEX_WAIT_BITSET: i32 = 9;
    // const FUTEX_WAKE_BITSET: i32 = 10;
    let futex_op = futex_op & !FUTEX_PRIVATE_FLAG;
    let task = current_task();
    uaddr.check(&task)?;
    match futex_op {
        FUTEX_WAIT => {
            if uaddr.read() != val {
                return Err(SysError::EAGAIN);
            }
            let future = FutexFuture {
                uaddr,
                val,
                in_futexes: false,
            };
            let timeout = UserReadPtr::<TimeSpec>::from(timeout as usize);
            if timeout.is_null() {
                future.await;
            } else {
                let timeout = timeout.read(&task)?;
                TimeLimitedTaskFuture::new(timeout.into(), future).await;
            }
        }
        FUTEX_WAKE => {
            let n_wake = task.with_mut_futexes(|futexes| futexes.wake(uaddr.raw(), val));
            yield_now().await;
            return Ok(n_wake);
        }
        FUTEX_CMP_REQUEUE => {
            if uaddr.read() != val3 {
                return Err(SysError::EAGAIN);
            }
            // const struct timespec *timeout,   /* or: uint32_t val2 */
            task.with_mut_futexes(|futexes| {
                futexes.requeue_waiters(uaddr.raw(), uaddr2, val, timeout)
            });
        }
        FUTEX_REQUEUE => {
            // const struct timespec *timeout,   /* or: uint32_t val2 */
            task.with_mut_futexes(|futexes| {
                futexes.requeue_waiters(uaddr.raw(), uaddr2, val, timeout)
            });
        }
        _ => return Err(SysError::ENOSYS),
    }
    Ok(0)
}

struct FutexFuture {
    uaddr: FutexWord,
    val: u32,
    in_futexes: bool,
}

impl Future for FutexFuture {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let task = current_task();
        task.with_mut_futexes(|futexes| {
            if !self.in_futexes {
                futexes.add_waiter(self.uaddr.raw(), task.tid(), cx.waker().clone());
                self.in_futexes = true;
                if self.uaddr.read() == self.val {
                    return Poll::Pending;
                    // 一旦返回Pending就会被移出调度队列，直到被wake
                } else {
                    return Poll::Ready(());
                };
            }
            // task is waked and will run the following code
            futexes.remove_waiter(self.uaddr.raw(), task.tid());
            Poll::Ready(())
        })
    }
}

/// actually this syscall has no actual effect
pub fn sys_get_robust_list(
    pid: i32,
    robust_list_head: UserWritePtr<RobustListHead>,
    len_ptr: UserWritePtr<usize>,
) -> SyscallResult {
    let Some(task) = TASK_MANAGER.get(pid as usize) else {
        return Err(SysError::ESRCH);
    };
    if !task.is_leader() {
        return Err(SysError::ESRCH);
    }
    // UserReadPtr::<RobustListHead>::from(value)
    len_ptr.write(&task, mem::size_of::<RobustListHead>())?;
    robust_list_head.write(&task, unsafe {
        *task.with_futexes(|futexes| futexes.robust_list.load(Ordering::SeqCst))
    })?;
    Ok(0)
}

/// actually this syscall has no actual effect
pub fn sys_set_robust_list(
    robust_list_head: UserReadPtr<RobustListHead>,
    len: usize,
) -> SyscallResult {
    let task = current_task();
    if len != mem::size_of::<RobustListHead>() {
        return Err(SysError::EINVAL);
    }
    let mut head = robust_list_head.into_ref(&task)?;
    task.with_mut_futexes(|futexes| futexes.robust_list.store(head.ptr_mut(), Ordering::SeqCst));
    Ok(0)
}
