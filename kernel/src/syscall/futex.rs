use async_utils::suspend_now;
use bitflags::Flags;
use systype::{SysError, SyscallResult};
use time::timespec::TimeSpec;

use super::Syscall;
use crate::{
    ipc::futex::{futex_manager, FutexHashKey, FutexOp, FutexWaiter, RobustListHead},
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
        let mut futex_op = FutexOp::from_bits_truncate(futex_op);
        let task = self.task;
        uaddr.check(&task)?;
        let is_private = futex_op.contains(FutexOp::Private);
        futex_op.remove(FutexOp::Private);
        let key = if is_private {
            FutexHashKey::Private {
                mm: task.raw_mm_pointer(),
                virtaddr: uaddr.addr,
            }
        } else {
            let phyaddr = task.with_memory_space(|mm| mm.va2pa(uaddr.addr));
            FutexHashKey::Shared { phyaddr }
        };
        log::warn!(
            "[sys_futex] {:?} uaddr:{:#x} key:{:?}",
            futex_op,
            uaddr.raw(),
            key
        );

        match futex_op {
            FutexOp::Wait => {
                let res = uaddr.read();
                if res != val {
                    log::warn!(
                        "[futex_wait] value in {} addr is {res} but expect {val}",
                        uaddr.addr.0
                    );
                    return Err(SysError::EAGAIN);
                }
                futex_manager().add_waiter(
                    &key,
                    FutexWaiter {
                        tid: task.tid(),
                        waker: task.waker().clone().unwrap(),
                    },
                );
                task.set_interruptable();
                let wake_up_signal = !*task.sig_mask_ref();
                task.set_wake_up_signal(wake_up_signal);
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
                        futex_manager().remove_waiter(&key, task.tid());
                    }
                }
                if task.with_sig_pending(|p| p.has_expect_signals(wake_up_signal)) {
                    log::warn!("[sys_futex] Woken by signal");
                    futex_manager().remove_waiter(&key, task.tid());
                    return Err(SysError::EINTR);
                }
                log::warn!("[sys_futex] I was woken");
                task.set_running();
                Ok(0)
            }
            FutexOp::Wake => {
                let n_wake = futex_manager().wake(&key, val)?;
                return Ok(n_wake);
            }
            FutexOp::Requeue => {
                let n_wake = futex_manager().wake(&key, val)?;
                let new_key = if is_private {
                    FutexHashKey::Private {
                        mm: task.raw_mm_pointer(),
                        virtaddr: uaddr2.into(),
                    }
                } else {
                    let phyaddr = task.with_memory_space(|mm| mm.va2pa(uaddr2.into()));
                    FutexHashKey::Shared { phyaddr }
                };
                futex_manager().requeue_waiters(key, new_key, timeout)?;
                Ok(n_wake)
            }
            FutexOp::CmpRequeue => {
                if uaddr.read() as u32 != val3 {
                    return Err(SysError::EAGAIN);
                }
                let n_wake = futex_manager().wake(&key, val)?;
                let new_key = if is_private {
                    FutexHashKey::Private {
                        mm: task.raw_mm_pointer(),
                        virtaddr: uaddr2.into(),
                    }
                } else {
                    let phyaddr = task.with_memory_space(|mm| mm.va2pa(uaddr2.into()));
                    FutexHashKey::Shared { phyaddr }
                };
                futex_manager().requeue_waiters(key, new_key, timeout)?;
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
