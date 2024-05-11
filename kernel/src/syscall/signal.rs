use config::process::INIT_PROC_PID;
use signal::{
    action::{Action, ActionType},
    siginfo::{SigDetails, SigInfo},
    signal_stack::{SignalStack, UContext},
    sigset::{Sig, SigSet},
};
use systype::{SysError, SyscallResult};

use crate::{
    mm::{UserReadPtr, UserWritePtr},
    processor::hart::current_task,
    task::{
        signal::{SigAction, WaitExpectSignals, SIG_DFL, SIG_IGN},
        TASK_MANAGER,
    },
};

/// 功能：为当前进程设置某种信号的处理函数，同时保存设置之前的处理函数。
/// 参数：signum 表示信号的编号，action 表示要设置成的处理函数的指针
/// old_action 表示用于保存设置之前的处理函数的指针。
/// 返回值：如果传入参数错误（比如传入的 action 或 old_action
/// 为空指针或者） 信号类型不存在返回 -1 ，否则返回 0 。
/// syscall ID: 134
///
/// NOTE: sigaction() can be called with a NULL second argument to query the
/// current signal handler. It can also be used to check whether a given signal
/// is valid for the current machine by calling it with NULL second and third
/// arguments.
pub fn sys_sigaction(
    signum: i32,
    action: UserReadPtr<SigAction>,
    old_action: UserWritePtr<SigAction>,
) -> SyscallResult {
    let task = current_task();
    let signum = Sig::from_i32(signum);
    if !signum.is_valid() {
        return Err(SysError::EINVAL);
    }
    if action.is_null() {
        if old_action.is_null() {
            return Ok(0);
        }
        let old = task.sig_handlers().get(signum);
        old_action.write(&task, old.into())?;
        return Ok(0);
    }
    let action = action.read(&task)?;
    let new = Action {
        atype: match action.sa_handler {
            SIG_DFL => ActionType::default(signum),
            SIG_IGN => ActionType::Ignore,
            entry => ActionType::User { entry },
        },
        flags: action.sa_flags,
        mask: action.sa_mask,
    };
    task.sig_handlers().update(signum, new);
    // TODO: 这里删掉了UMI的一点东西？不知道会不会影响
    if !old_action.is_null() {
        let old = task.sig_handlers().get(signum);
        old_action.write(&task, old.into())?;
    }
    Ok(0)
}

/// how决定如何修改当前的信号屏蔽字;set指定了需要添加、移除或设置的信号;
/// 当前的信号屏蔽字会被保存在 oldset 指向的位置
pub fn sys_sigprocmask(
    how: usize,
    set: UserReadPtr<SigSet>,
    old_set: UserWritePtr<SigSet>,
) -> SyscallResult {
    const SIGBLOCK: usize = 0;
    const SIGUNBLOCK: usize = 1;
    const SIGSETMASK: usize = 2;
    let task = current_task();
    if !old_set.is_null() {
        old_set.write(&task, *task.sig_mask())?;
    }
    if !set.is_null() {
        let set = set.read(&task)?;
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

pub fn sys_sigreturn() -> SyscallResult {
    let task = current_task();
    let cx = task.trap_context_mut();
    let ucontext_ptr = UserReadPtr::<UContext>::from(task.sig_ucontext_ptr());
    log::trace!("[sys_sigreturn] ucontext_ptr: {ucontext_ptr:?}");
    let ucontext = ucontext_ptr.read(&task)?;
    *task.sig_mask() = ucontext.uc_sigmask;
    *task.signal_stack() = (ucontext.uc_stack.ss_size != 0).then_some(ucontext.uc_stack);
    cx.sepc = ucontext.uc_mcontext.sepc;
    cx.user_x = ucontext.uc_mcontext.user_x;
    Ok(cx.user_x[10])
}

pub fn sys_signalstack(
    _ss: UserReadPtr<SignalStack>,
    old_ss: UserWritePtr<SignalStack>,
) -> SyscallResult {
    if !old_ss.is_null() {
        // old_ss.write(current_task(), current_task())
    }
    todo!()
}

/// The kill() system call can be used to send any signal to any process group
/// or process.
/// - If pid is positive, then signal sig is sent to the process with the ID
///   specified by pid.
/// - If pid equals 0, then sig is sent to every process in the process group of
///   the calling process.
/// - If pid equals -1, then sig is sent to every process for which the calling
///   process has permission to send signals, except for process 1 (init)
/// - If pid is less than -1, then sig is sent to every process in the process
///   group whose ID is -pid.
/// - If sig is 0, then no signal is sent, but existence and permission checks
///   are still performed; this can be used to check for the existence of a
///   process ID or process group ID that the caller is permitted to signal.
///
/// **RETURN VALUE** :On success (at least one signal was sent), zero is
/// returned. On error, -1 is returned, and errno is set appropriately
pub fn sys_kill(pid: isize, signum: i32) -> SyscallResult {
    let sig = Sig::from_i32(signum);
    if !sig.is_valid() {
        return Err(SysError::EINVAL);
    }
    // log::debug!("[sys_kill] signal {sig:?}");
    match pid {
        0 => {
            // 进程组
            // unimplemented!()
            let pid = current_task().pid();
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

/// sends the signal sigum to the thread with the thread ID tid in the thread
/// group tgid.  (By contrast, kill(2) can be used to send a signal only to a
/// process (i.e., thread group) as a whole, and the signal will be delivered to
/// an arbitrary thread within that process.)
pub fn sys_tgkill(tgid: isize, tid: isize, signum: i32) -> SyscallResult {
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
/// to be specified, which may result in the wrong thread being signaled if a
/// thread terminates and its thread ID is recycled.  Avoid using this system
/// call.
pub fn sys_tkill(tid: isize, signum: i32) -> SyscallResult {
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
/// given by mask and then suspends the thread until delivery of a signal whose
/// action is to invoke a signal handler or to terminate a process
///
/// If the signal terminates the process, then sigsuspend() does not return.  If
/// the signal is caught, then sigsuspend() returns after the signal handler
/// returns, and the signal mask is restored to the state before the call to
/// sigsuspend().
///
/// It is not possible to block SIGKILL or SIGSTOP; specifying these signals in
/// mask, has no effect on the thread's signal mask.
pub async fn sys_sigsuspend(mask: UserReadPtr<SigSet>) -> SyscallResult {
    let task = current_task();
    let mut mask = mask.read(&task)?;
    let oldmask = task.sig_mask_replace(&mut mask);
    WaitExpectSignals::new(&task, *task.sig_mask()).await;
    // TODO: 根据Linux这里理论上应该等到signal
    // handler返回时sys_sigsuspend再返回，但是貌似其他队都没有这样做
    *task.sig_mask() = oldmask;
    Err(SysError::EINTR)
}
