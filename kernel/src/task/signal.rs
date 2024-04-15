use alloc::sync::Arc;
use core::alloc::Layout;

use arch::sstatus::{self, Sstatus};
use memory::VirtAddr;
use signal::{
    action::{Action, ActionType},
    signal_stack::{self, MContext, SignalStack, UContext},
    sigset::{Sig, SigSet},
};

use super::Task;
use crate::{
    mm::{Page, UserWritePtr},
    processor::hart::{current_task, current_trap_cx},
    trap::{ctx::UserFloatContext, TrapContext},
};

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

// pub struct SignalContext {
// pub blocked: SigSet,
// general regs[0..31]
// pub user_x: [usize; 32],
// general float regs
// pub user_fx: UserFloatContext,
// CSR sepc
// pub sepc: usize, // 33
// }
//
// impl SignalContext {
// pub fn new(blocked: SigSet, trap_context: &TrapContext) -> Self {
// let mut sstatus = sstatus::read();
// Self {
// blocked,
// user_x: trap_context.user_x,
// user_fx: trap_context.user_fx,
// sepc: trap_context.sepc,
// }
// }
// }
//
// pub struct SignalTrapoline {
// page: Arc<Page>,
// user_addr: VirtAddr,
// }
//
// impl SignalTrapoline {
// pub fn new(task: Arc<Task>) -> Self{
//
// }
// }
extern "C" {
    fn sigreturn_trampoline();
}

pub fn do_signal() {
    let mut signal = current_task().signal.lock();
    // if there is no signal to be handle, just return
    if signal.pending.is_empty() {
        return;
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
                let ucontext_ptr = save_context_into_sigstack(old_blocked);
                // 用户自定义的sa_handler的参数，void myhandler(int signo,siginfo_t *si,void
                // *ucontext); TODO:实现siginfo
                // a0
                current_trap_cx().user_x[10] = sig.raw();
                // a2
                current_trap_cx().user_x[12] = ucontext_ptr;
                current_trap_cx().sepc = entry;
                // ra (when the sigaction set by user finished,if user forgets to call
                // sys_sigretrun, it will return to sigreturn_trampoline, which
                // calls sys_sigreturn)
                current_trap_cx().user_x[1] = sigreturn_trampoline as usize;
                // sp (it will be used later by sys_sigreturn)
                current_trap_cx().user_x[2] = ucontext_ptr;
            }
        }
    }
}

fn ignore(sig: Sig) {
    log::debug!("ignore this sig {}", sig);
}
fn terminate(sig: Sig) {
    log::info!("terminate this sig {}", sig);
}
fn stop(sig: Sig) {
    log::info!("stop this sig {}", sig);
}
fn cont(sig: Sig) {
    log::info!("cont this sig {}", sig);
}

fn save_context_into_sigstack(old_blocked: SigSet) -> usize {
    let trap_context = current_trap_cx();
    trap_context.user_fx.encounter_signal();
    let signal_stack = current_task().signal_stack().take();
    let stack_top = match signal_stack {
        Some(s) => s.get_stack_top(),
        None => current_trap_cx().kernel_sp,
    };
    // extend the signal_stack
    let pad_ucontext = Layout::new::<UContext>().pad_to_align().size();
    let ucontext_ptr = UserWritePtr::<UContext>::from(stack_top - pad_ucontext);
    // TODO: should increase the size of the signal_stack? It seams umi doesn't do
    // that
    let mut ucontext = UContext {
        uc_link: 0,
        uc_sigmask: old_blocked,
        uc_stack: signal_stack.unwrap_or_default(),
        uc_mcontext: MContext {
            sepc: trap_context.sepc,
            user_x: trap_context.user_x,
        },
    };
    let ptr = ucontext_ptr.as_usize();
    ucontext_ptr.write(current_task(), ucontext);
    ptr
}
