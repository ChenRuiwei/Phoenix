use core::mem;

use async_utils::suspend_now;
use config::process::INIT_PROC_PID;
use signal::{
    action::{Action, ActionType},
    siginfo::{SigDetails, SigInfo},
    signal_stack::{SignalStack, UContext},
    sigset::{Sig, SigSet},
};
use systype::{SysError, SyscallResult};
use time::timespec::TimeSpec;

use super::Syscall;
use crate::{
    mm::{UserReadPtr, UserWritePtr},
    task::{
        signal::{SigAction, SIG_DFL, SIG_IGN},
        TASK_MANAGER,
    },
};

impl Syscall<'_> {
    /// NOTE: sigaction() can be called with a NULL second argument to query the
    /// current signal handler. It can also be used to check whether a given
    /// signal is valid for the current machine by calling it with NULL
    /// second and third arguments.
    pub fn sys_rt_sigaction(
        &self,
        signum: i32,
        action: UserReadPtr<SigAction>,
        old_action: UserWritePtr<SigAction>,
    ) -> SyscallResult {
        let task = self.task;
        let signum = Sig::from_i32(signum);
        // 不能为SIGKILL或者SIGSTOP绑定处理函数
        if !signum.is_valid() || signum.is_kill_or_stop() {
            return Err(SysError::EINVAL);
        }
        log::info!(
            "[sys_rt_sigaction] {signum:?}, new_ptr:{action}, old_ptr:{old_action}, old_sa_type:{:?}",
            task.with_sig_handlers(|handlers| { handlers.get(signum).atype })
        );
        if old_action.not_null() {
            let old = task.with_sig_handlers(|handlers| handlers.get(signum));
            old_action.write(&task, old.into())?;
        }
        if action.not_null() {
            let mut action = action.read(&task)?;
            // 无法在一个信号处理函数执行的时候屏蔽调SIGKILL和SIGSTOP信号
            action.sa_mask.remove(SigSet::SIGKILL | SigSet::SIGSTOP);
            let new = Action {
                atype: match action.sa_handler {
                    SIG_DFL => ActionType::default(signum),
                    SIG_IGN => ActionType::Ignore,
                    entry => ActionType::User { entry },
                },
                flags: action.sa_flags,
                mask: action.sa_mask,
            };
            log::info!("[sys_rt_sigaction] new:{:?}", new);
            task.with_mut_sig_handlers(|handlers| handlers.update(signum, new));
        }
        Ok(0)
    }

    /// how决定如何修改当前的信号屏蔽字;set指定了需要添加、移除或设置的信号;
    /// 当前的信号屏蔽字会被保存在 oldset 指向的位置
    /// The use of sigprocmask() is unspecified in a multithreaded process;
    pub fn sys_rt_sigprocmask(
        &self,
        how: usize,
        set: UserReadPtr<SigSet>,
        old_set: UserWritePtr<SigSet>,
        sigset_size: usize,
    ) -> SyscallResult {
        const SIGBLOCK: usize = 0;
        const SIGUNBLOCK: usize = 1;
        const SIGSETMASK: usize = 2;
        debug_assert!(sigset_size == 8);
        let task = self.task;
        if old_set.not_null() {
            old_set.write(&task, *task.sig_mask())?;
        }
        if set.not_null() {
            let mut set = set.read(&task)?;
            log::info!("[sys_rt_sigprocmask] set:{set:#x}");
            // It is not possible to block SIGKILL or SIGSTOP.  Attempts to do so are
            // silently ignored.
            set.remove(SigSet::SIGKILL | SigSet::SIGCONT);
            match how {
                SIGBLOCK => {
                    *task.sig_mask() |= set;
                }
                SIGUNBLOCK => {
                    task.sig_mask().remove(set);
                }
                SIGSETMASK => {
                    *task.sig_mask() = set;
                }
                _ => {
                    return Err(SysError::EINVAL);
                }
            };
        }
        Ok(0)
    }

    // NOTE: should return a0 of the saved user context, since signal handler has
    // preempted the return of the last trap call.
    pub fn sys_rt_sigreturn(&self) -> SyscallResult {
        let task = self.task;
        let cx = task.trap_context_mut();
        let ucontext_ptr: UserReadPtr<UContext> = (task.sig_ucontext_ptr()).into();
        // log::trace!("[sys_rt_sigreturn] ucontext_ptr: {ucontext_ptr:?}");
        let ucontext = ucontext_ptr.read(&task)?;
        // log::error!("[SA_SIGINFO] load ucontext {ucontext:?}");
        *task.sig_mask() = ucontext.uc_sigmask;
        *task.sig_stack() = (ucontext.uc_stack.ss_size != 0).then_some(ucontext.uc_stack);
        cx.sepc = ucontext.uc_mcontext.user_x[0];
        cx.user_x = ucontext.uc_mcontext.user_x;
        // log::error!("stask after {:#x}", cx.user_x[2]);
        Ok(cx.user_x[10])
    }

    pub fn sys_rt_signalstack(
        &self,
        _ss: UserReadPtr<SignalStack>,
        old_ss: UserWritePtr<SignalStack>,
    ) -> SyscallResult {
        if !old_ss.is_null() {
            // old_ss.write(self.task, current_task())
        }
        todo!()
    }

    /// The kill() system call can be used to send any signal to any process
    /// group or process.
    /// - If pid is positive, then signal sig is sent to the process with the ID
    ///   specified by pid.
    /// - If pid equals 0, then sig is sent to every process in the process
    ///   group of the calling process.
    /// - If pid equals -1, then sig is sent to every process for which the
    ///   calling process has permission to send signals, except for process 1
    ///   (init)
    /// - If pid is less than -1, then sig is sent to every process in the
    ///   process group whose ID is -pid.
    /// - If sig is 0, then no signal is sent, but existence and permission
    ///   checks are still performed; this can be used to check for the
    ///   existence of a process ID or process group ID that the caller is
    ///   permitted to signal.
    ///
    /// **RETURN VALUE** :On success (at least one signal was sent), zero is
    /// returned. On error, -1 is returned, and errno is set appropriately
    pub fn sys_kill(&self, pid: isize, signum: i32) -> SyscallResult {
        if signum == 0 {
            log::warn!("signum is zero, currently skip the permission check");
            return Ok(0);
        }
        let sig = Sig::from_i32(signum);
        if !sig.is_valid() {
            return Err(SysError::EINVAL);
        }
        // log::debug!("[sys_kill] signal {sig:?}");
        match pid {
            0 => {
                // 进程组
                // unimplemented!()
                let pid = self.task.pid();
                if let Some(task) = TASK_MANAGER.get(pid as usize) {
                    if task.is_leader() {
                        task.receive_siginfo(
                            SigInfo {
                                sig,
                                code: SigInfo::USER,
                                details: SigDetails::Kill { pid },
                            },
                            false,
                        );
                    } else {
                        // sys_kill is sent to process not thread
                        return Err(SysError::ESRCH);
                    }
                } else {
                    return Err(SysError::ESRCH);
                }
            }
            -1 => {
                TASK_MANAGER.for_each(|task| {
                    if task.pid() != INIT_PROC_PID && task.is_leader() && sig.raw() != 0 {
                        task.receive_siginfo(
                            SigInfo {
                                sig,
                                code: SigInfo::USER,
                                details: SigDetails::Kill { pid: task.pid() },
                            },
                            false,
                        );
                    }
                    Ok(())
                })?;
            }
            _ if pid > 0 => {
                if let Some(task) = TASK_MANAGER.get(pid as usize) {
                    if task.is_leader() {
                        task.receive_siginfo(
                            SigInfo {
                                sig,
                                code: SigInfo::USER,
                                details: SigDetails::Kill { pid: task.pid() },
                            },
                            false,
                        );
                    } else {
                        // sys_kill is sent to process not thread
                        return Err(SysError::ESRCH);
                    }
                } else {
                    return Err(SysError::ESRCH);
                }
            }
            _ => {
                // pid < -1
                // sig is sent to every process in the process group whose ID is -pid.
                unimplemented!()
            }
        }
        Ok(0)
    }

    /// sends the signal sigum to the thread with the thread ID tid in the
    /// thread group tgid.  (By contrast, kill(2) can be used to send a
    /// signal only to a process (i.e., thread group) as a whole, and the
    /// signal will be delivered to an arbitrary thread within that
    /// process.)
    pub fn sys_tgkill(&self, tgid: isize, tid: isize, signum: i32) -> SyscallResult {
        let sig = Sig::from_i32(signum);
        if !sig.is_valid() || tgid < 0 || tid < 0 {
            return Err(SysError::EINVAL);
        }
        let task = TASK_MANAGER.get(tgid as usize).ok_or(SysError::ESRCH)?;
        if !task.is_leader() {
            return Err(SysError::ESRCH);
        }
        task.with_mut_thread_group(|tg| -> SyscallResult {
            for thread in tg.iter() {
                if thread.tid() == tid as usize {
                    thread.receive_siginfo(
                        SigInfo {
                            sig,
                            code: SigInfo::TKILL,
                            details: SigDetails::Kill { pid: task.pid() },
                        },
                        true,
                    );
                    return Ok(0);
                }
            }
            return Err(SysError::ESRCH);
        })
    }

    /// An obsolete predecessor to tgkill(). It allows only the target thread ID
    /// to be specified, which may result in the wrong thread being signaled if
    /// a thread terminates and its thread ID is recycled.  Avoid using this
    /// system call.
    pub fn sys_tkill(&self, tid: isize, signum: i32) -> SyscallResult {
        let sig = Sig::from_i32(signum);
        if !sig.is_valid() || tid < 0 {
            return Err(SysError::EINVAL);
        }
        let task = TASK_MANAGER.get(tid as usize).ok_or(SysError::ESRCH)?;
        task.receive_siginfo(
            SigInfo {
                sig,
                code: SigInfo::TKILL,
                details: SigDetails::Kill { pid: task.pid() },
            },
            true,
        );
        Ok(0)
    }

    /// temporarily replaces the signal mask of the calling thread with the mask
    /// given by mask and then suspends the thread until delivery of a signal
    /// whose action is to invoke a signal handler or to terminate a process
    ///
    /// If the signal terminates the process, then sigsuspend() does not return.
    /// If the signal is caught, then sigsuspend() returns after the signal
    /// handler returns, and the signal mask is restored to the state before
    /// the call to sigsuspend().
    ///
    /// It is not possible to block SIGKILL or SIGSTOP; specifying these signals
    /// in mask, has no effect on the thread's signal mask.
    pub async fn sys_rt_sigsuspend(&self, mask: UserReadPtr<SigSet>) -> SyscallResult {
        let task = self.task;
        let mut mask = mask.read(&task)?;
        mask.remove(SigSet::SIGKILL | SigSet::SIGSTOP);
        let oldmask = mem::replace(task.sig_mask(), mask);
        let invoke_signal = task.with_sig_handlers(|handlers| handlers.bitmap());
        task.set_interruptable();
        task.set_wake_up_signal(mask | invoke_signal);
        suspend_now().await;
        *task.sig_mask() = oldmask;
        task.set_running();
        Err(SysError::EINTR)
    }

    /// Suspends execution of the calling thread until one of the signals in set
    /// is pending (If one of the signals in set is already pending for the
    /// calling thread, sigwaitinfo() will return immediately.). It removes the
    /// signal from the set of pending signals and returns the signal number
    /// as its function result.
    ///
    /// - `set`: Suspend the execution of the process until a signal in `set`
    ///   that arrives
    /// - `info`: If it is not NULL, the buffer that it points to is used to
    ///   return a structure of type siginfo_t containing information about the
    ///   signal.
    /// - `timeout`: specifies the interval for which the thread is suspended
    ///   waiting for a signal.
    ///
    /// On success, sigtimedwait() returns a signal number
    pub async fn sys_rt_sigtimedwait(
        &self,
        set: UserReadPtr<SigSet>,
        info: UserWritePtr<SigInfo>,
        timeout: UserReadPtr<TimeSpec>,
    ) -> SyscallResult {
        let task = self.task;
        let mut set = set.read(&task)?;
        set.remove(SigSet::SIGKILL | SigSet::SIGSTOP);

        task.set_interruptable();
        task.set_wake_up_signal(set);
        if timeout.not_null() {
            let timeout = timeout.read(&task)?;
            if !timeout.is_valid() {
                return Err(SysError::EINVAL);
            }
            log::warn!("[sys_rt_sigtimedwait] {:?}", timeout);
            task.suspend_timeout(timeout.into()).await;
        } else {
            suspend_now().await;
        }

        task.set_running();
        let si = task.with_mut_sig_pending(|pending| pending.dequeue_expect(set));
        if let Some(si) = si {
            log::warn!("[sys_rt_sigtimedwait] I'm woken by {:?}", si);
            if info.not_null() {
                info.write(&task, si)?;
            }
            Ok(si.sig.raw())
        } else {
            log::warn!("[sys_rt_sigtimedwait] I'm woken by timeout");
            Err(SysError::EAGAIN)
        }
    }
}
