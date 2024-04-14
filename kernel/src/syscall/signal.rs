use rand_core::le;
use signal::{
    action::{Action, ActionType},
    signal_stack::{SignalStack, UContext},
    sigset::{Sig, SigProcMaskHow, SigSet},
};
use systype::{SysError, SyscallResult};

use crate::{
    mm::{UserReadPtr, UserWritePtr},
    processor::hart::{current_task, current_trap_cx},
    task::signal::{SigAction, SIG_DFL, SIG_IGN},
};

/// 功能：为当前进程设置某种信号的处理函数，同时保存设置之前的处理函数。
/// 参数：signum 表示信号的编号，action 表示要设置成的处理函数的指针
/// old_action 表示用于保存设置之前的处理函数的指针。
/// 返回值：如果传入参数错误（比如传入的 action 或 old_action
/// 为空指针或者） 信号类型不存在返回 -1 ，否则返回 0 。
/// syscall ID: 134
pub fn sys_sigaction(
    signum: Sig,
    action: UserReadPtr<SigAction>,
    old_action: UserWritePtr<SigAction>,
) -> SyscallResult {
    if !signum.is_valid() {
        return Err(SysError::EINVAL);
    }
    let action = action.read(current_task())?;
    let new = Action {
        atype: match action.sa_handler {
            SIG_DFL => ActionType::default(signum),
            SIG_IGN => ActionType::Ignore,
            entry => ActionType::User {
                entry: entry.into(),
            },
        },
        flags: action.sa_flags,
        mask: action.sa_mask,
    };
    let mut signal = current_task().signal.lock();
    let old = signal.handlers.replace(signum, new);
    drop(signal);

    // TODO：这里删掉了UMI的一点东西？不知道会不会影响
    if !old_action.is_null() {
        old_action.write(current_task(), old.into());
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
    let mut signal = current_task().signal.lock();
    if !old_set.is_null() {
        old_set.write(current_task(), signal.blocked);
    }
    if !set.is_null() {
        match SigProcMaskHow::from(how) {
            SigProcMaskHow::SigBlock => {
                signal.blocked |= set.read(current_task())?;
            }
            SigProcMaskHow::SigUnblock => {
                signal.blocked.remove(set.read(current_task())?);
            }
            SigProcMaskHow::SigSetMask => {
                signal.blocked = set.read(current_task())?;
            }
            SigProcMaskHow::Unknown => {
                return Err(SysError::EINVAL);
            }
        }
    }
    Ok(0)
}

pub fn sys_sigreturn() -> SyscallResult {
    let ucontext_ptr = UserReadPtr::<UContext>::from_usize(current_trap_cx().user_x[1]);
    // TODO: if can't read, it should cause segment fault
    let ucontext = ucontext_ptr.read(current_task())?;
    current_task().signal.lock().blocked = ucontext.uc_sigmask;
    current_task().set_signal_stack((ucontext.uc_stack.ss_size != 0).then_some(ucontext.uc_stack));
    current_trap_cx().sepc = ucontext.uc_mcontext.sepc;
    current_trap_cx().user_x = ucontext.uc_mcontext.user_x;
    Ok(0)
}

pub fn sys_signalstack(ss: UserReadPtr::<SignalStack>, old_ss: UserWritePtr::<SignalStack>) -> SyscallResult {
    if !old_ss.is_null() {
        // old_ss.write(current_task(), current_task())
    }
    todo!()
}
