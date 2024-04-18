use alloc::sync::Arc;
use core::{
    alloc::Layout,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use arch::time::get_time_ms;
use signal::{
    action::{Action, ActionType},
    signal_stack::{MContext, UContext},
    sigset::{Sig, SigSet},
};
use systype::SysResult;

use super::Task;
use crate::{mm::UserWritePtr, processor::hart::current_task};

#[derive(Clone, Copy)]
#[repr(C)]
pub struct SigAction {
    /// sa_handler specifies the action to be associated with signum and can be
    /// one of the following:
    /// 1. SIG_DFL for the default action
    /// 2. SIG_IGN to ignore this signal
    /// 3. A pointer to a signal handling function. This function receives the
    ///    signal number as its only argument.
    pub sa_handler: usize,
    /// sa_mask specifies a mask of signals which should be blocked during
    /// execution of the signal handler.
    pub sa_mask: SigSet,
    pub sa_flags: usize,
}
pub const SIG_DFL: usize = 0;
pub const SIG_IGN: usize = 1;

impl Default for SigAction {
    fn default() -> Self {
        SigAction {
            sa_handler: 0,
            sa_mask: SigSet::empty(),
            sa_flags: Default::default(),
        }
    }
}
impl From<Action> for SigAction {
    fn from(action: Action) -> Self {
        match action.atype {
            ActionType::Ignore => SigAction {
                sa_handler: SIG_IGN,
                ..Default::default()
            },
            ActionType::Kill | ActionType::Stop | ActionType::Cont => SigAction {
                sa_handler: SIG_DFL,
                ..Default::default()
            },
            ActionType::User { entry } => SigAction {
                sa_handler: entry.into(),
                ..Default::default()
            },
        }
    }
}

extern "C" {
    fn sigreturn_trampoline();
}

pub fn do_signal() -> SysResult<()> {
    let task = current_task();
    task.with_mut_signal(|signal| -> SysResult<()> {
        // if there is no signal to be handle, just return
        if signal.pending.is_empty() {
            return Ok(());
        }
        let len = signal.pending.queue.len();
        for _ in 0..len {
            let sig = signal.pending.pop().unwrap();
            if !sig.is_kill_or_stop() && signal.blocked.contain_signal(sig) {
                signal.pending.add(sig);
                continue;
            }
            let action = signal.handlers.get(sig).unwrap().clone();
            match action.atype {
                ActionType::Ignore => ignore(sig),
                ActionType::Kill => terminate(sig),
                ActionType::Stop => stop(sig),
                ActionType::Cont => cont(sig),
                ActionType::User { entry } => {
                    // 在跳转到用户定义的信号处理程序之前，内核需要保存当前进程的上下文，
                    // 包括程序计数器、寄存器状态、栈指针等。此外，当前的信号屏蔽集也需要被保存，
                    // 因为信号处理程序可能会被嵌套调用（即一个信号处理程序中可能会触发另一个信号），
                    // 所以需要确保每个信号处理程序能恢复到它被调用时的屏蔽集状态
                    let old_blocked = signal.blocked;
                    // 在执行用户定义的信号处理程序之前，内核会将当前处理的信号添加到信号屏蔽集中。
                    // 这样做是为了防止在处理该信号的过程中，相同的信号再次中断
                    signal.blocked.add_signal(sig);
                    // 信号定义中可能包含了在处理该信号时需要阻塞的其他信号集。
                    // 这些信息定义在Action的mask字段
                    signal.blocked |= action.mask;
                    let ucontext_ptr = save_context_into_sigstack(old_blocked)?;
                    let trap_cx = task.trap_context_mut();
                    // 用户自定义的sa_handler的参数，void myhandler(int signo,siginfo_t *si,void
                    // *ucontext); TODO:实现siginfo
                    // a0
                    trap_cx.user_x[10] = sig.raw();
                    // a2
                    trap_cx.user_x[12] = ucontext_ptr;
                    trap_cx.sepc = entry;
                    // ra (when the sigaction set by user finished,if user forgets to call
                    // sys_sigreturn, it will return to sigreturn_trampoline, which
                    // calls sys_sigreturn)
                    trap_cx.user_x[1] = sigreturn_trampoline as usize;
                    // sp (it will be used later by sys_sigreturn)
                    trap_cx.user_x[2] = ucontext_ptr;
                }
            }
        }
        Ok(())
    })
}

fn ignore(sig: Sig) {
    log::debug!("Recevie signal {}. Action: ignore", sig);
}
fn terminate(sig: Sig) {
    log::info!("Recevie signal {}. Action: terminate", sig);
}
fn stop(sig: Sig) {
    log::info!("Recevie signal {}. Action: stop", sig);
}
fn cont(sig: Sig) {
    log::info!("Recevie signal {}. Action: continue", sig);
}

fn save_context_into_sigstack(old_blocked: SigSet) -> SysResult<usize> {
    let task = current_task();
    let trap_context = task.trap_context_mut();
    trap_context.user_fx.encounter_signal();
    let signal_stack = task.signal_stack().take();
    let stack_top = match signal_stack {
        Some(s) => s.get_stack_top(),
        None => trap_context.kernel_sp,
    };
    // extend the signal_stack
    let pad_ucontext = Layout::new::<UContext>().pad_to_align().size();
    let ucontext_ptr = UserWritePtr::<UContext>::from(stack_top - pad_ucontext);
    // TODO: should increase the size of the signal_stack? It seams umi doesn't do
    // that
    let ucontext = UContext {
        uc_link: 0,
        uc_sigmask: old_blocked,
        uc_stack: signal_stack.unwrap_or_default(),
        uc_mcontext: MContext {
            sepc: trap_context.sepc,
            user_x: trap_context.user_x,
        },
    };
    let ptr = ucontext_ptr.as_usize();
    ucontext_ptr.write(task, ucontext)?;
    Ok(ptr)
}

pub struct WaitHandlableSignal(pub &'static Arc<Task>);

impl Future for WaitHandlableSignal {
    type Output = usize;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        self.0.with_mut_signal(|signal| -> Poll<Self::Output> {
            match signal.pending.has_signal_to_handle(signal.blocked) {
                true => Poll::Ready(get_time_ms()),
                false => Poll::Pending,
            }
        })
    }
}
