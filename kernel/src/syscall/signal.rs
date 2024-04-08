use signal::action::SigAction;
use systype::SyscallResult;

/// 功能：为当前进程设置某种信号的处理函数，同时保存设置之前的处理函数。
/// 参数：signum 表示信号的编号，action 表示要设置成的处理函数的指针
/// old_action 表示用于保存设置之前的处理函数的指针（SignalAction
/// 结构稍后介绍）。 返回值：如果传入参数错误（比如传入的 action 或 old_action
/// 为空指针或者） 信号类型不存在返回 -1 ，否则返回 0 。
/// syscall ID: 134
pub fn sys_sigaction(
    sig: i32,
    action: *const SigAction,
    old_action: *mut SigAction,
) -> SyscallResult {
    Ok(0)
}
