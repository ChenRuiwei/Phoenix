use signal::{
    action::{Action, ActionType},
    signal_stack::{SignalStack, UContext},
    sigset::{Sig, SigSet},
};
use systype::{SysError, SyscallResult};

use crate::{
    mm::{UserReadPtr, UserWritePtr},
    processor::hart::current_task,
    task::signal::{SigAction, SIG_DFL, SIG_IGN},
};

/// 功能：为当前进程设置某种信号的处理函数，同时保存设置之前的处理函数。
/// 参数：signum 表示信号的编号，action 表示要设置成的处理函数的指针
/// old_action 表示用于保存设置之前的处理函数的指针。
/// 返回值：如果传入参数错误（比如传入的 action 或 old_action
/// 为空指针或者） 信号类型不存在返回 -1 ，否则返回 0 。
/// syscall ID: 134
pub fn sys_sigaction(
    signum: usize,
    action: UserReadPtr<SigAction>,
    old_action: UserWritePtr<SigAction>,
) -> SyscallResult {
    let task = current_task();
    let signum = Sig::from_usize(signum);
    if !signum.is_valid() {
        return Err(SysError::EINVAL);
    }
    let action = action.read(task)?;
    let new = Action {
        atype: match action.sa_handler {
            SIG_DFL => ActionType::default(signum),
            SIG_IGN => ActionType::Ignore,
            entry => ActionType::User { entry },
        },
        flags: action.sa_flags,
        mask: action.sa_mask,
    };
    let old = task.with_mut_signal(|signal| -> Action { signal.handlers.replace(signum, new) });
    // TODO: 这里删掉了UMI的一点东西？不知道会不会影响
    if !old_action.is_null() {
        old_action.write(task, old.into())?;
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
        task.with_signal(|signal| {
            old_set.write(task, signal.blocked)?;
            Ok(0)
        })?;
    }
    if set.is_null() {
        return Ok(0);
    }
    let set = set.read(task)?;
    task.with_mut_signal(|signal| -> SyscallResult {
        match how {
            SIGBLOCK => {
                signal.blocked |= set;
            }
            SIGUNBLOCK => {
                signal.blocked.remove(set);
            }
            SIGSETMASK => {
                signal.blocked = set;
            }
            _ => {
                return Err(SysError::EINVAL);
            }
        }
        Ok(0)
    })
}

pub fn sys_sigreturn() -> SyscallResult {
    let task = current_task();
    let trap_cx = task.trap_context_mut();
    let ucontext_ptr = UserReadPtr::<UContext>::from_usize(trap_cx.user_x[1]);

    let ucontext = ucontext_ptr.read(task)?;
    task.with_mut_signal(|signal| {
        signal.blocked = ucontext.uc_sigmask;
    });
    task.set_signal_stack((ucontext.uc_stack.ss_size != 0).then_some(ucontext.uc_stack));
    trap_cx.sepc = ucontext.uc_mcontext.sepc;
    trap_cx.user_x = ucontext.uc_mcontext.user_x;
    Ok(0)
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
