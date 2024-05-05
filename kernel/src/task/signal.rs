use alloc::sync::Arc;
use core::{
    alloc::Layout,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use arch::time::{get_time_duration, get_time_ms};
use signal::{
    action::{Action, ActionType},
    signal_stack::{MContext, UContext},
    sigset::{Sig, SigSet},
};
use systype::SysResult;
use time::timeval::ITimerVal;

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

impl Task {
    /// A signal may be process-directed or thread-directed
    /// A process-directed signal is targeted at  a  thread  group and is
    /// delivered to an arbitrarily selected thread from among those that are
    /// not blocking the signal
    /// A thread-directed signal is targeted at
    /// (i.e., delivered to) a specific thread.
    pub fn receive_signal(&self, sig: Sig, thread_directed: bool) {
        match thread_directed {
            false => {
                debug_assert!(self.is_leader());
                self.with_mut_thread_group(|tg| {
                    for task in tg.iter() {
                        if task.sig_mask().contain_signal(sig) {
                            continue;
                        }
                        task.with_mut_sig_pending(|pending| {
                            pending.add(sig);
                        });
                        break;
                    }
                })
            }
            true => {
                if self.sig_mask().contain_signal(sig) {
                    return;
                }
                self.with_mut_sig_pending(|pending| {
                    pending.add(sig);
                });
            }
        }
    }
}

extern "C" {
    fn sigreturn_trampoline();
}

pub fn do_signal() -> SysResult<()> {
    let task = current_task();
    task.with_mut_sig_pending(|pending| -> SysResult<()> {
        // if there is no signal to be handle, just return
        if pending.is_empty() {
            return Ok(());
        }
        log::info!("[do signal] there is some signals to be handled");
        let len = pending.queue.len();
        for _ in 0..len {
            let sig = pending.pop().unwrap();
            if !sig.is_kill_or_stop() && task.sig_mask().contain_signal(sig) {
                pending.add(sig);
                continue;
            }
            let action = task.sig_handlers().get(sig).unwrap().clone();
            match action.atype {
                ActionType::Ignore => ignore(sig),
                ActionType::Kill => terminate(sig),
                ActionType::Stop => stop(sig),
                ActionType::Cont => cont(sig),
                ActionType::User { entry } => {
                    log::info!("[do signal] user defined signal action");
                    // 在跳转到用户定义的信号处理程序之前，内核需要保存当前进程的上下文，
                    // 包括程序计数器、寄存器状态、栈指针等。此外，当前的信号屏蔽集也需要被保存，
                    // 因为信号处理程序可能会被嵌套调用（即一个信号处理程序中可能会触发另一个信号），
                    // 所以需要确保每个信号处理程序能恢复到它被调用时的屏蔽集状态
                    let old_mask = task.sig_mask();
                    // 在执行用户定义的信号处理程序之前，内核会将当前处理的信号添加到信号屏蔽集中。
                    // 这样做是为了防止在处理该信号的过程中，相同的信号再次中断
                    task.sig_mask().add_signal(sig);
                    // 信号定义中可能包含了在处理该信号时需要阻塞的其他信号集。
                    // 这些信息定义在Action的mask字段
                    task.sig_mask().add_signals(action.mask);

                    let ucontext_ptr = save_context_into_sigstack(*old_mask)?;
                    let cx = task.trap_context_mut();
                    // 用户自定义的sa_handler的参数，void myhandler(int signo,siginfo_t *si,void
                    // *ucontext); TODO:实现siginfo
                    // a0
                    cx.user_x[10] = sig.raw();
                    // a2
                    cx.user_x[12] = ucontext_ptr;
                    cx.sepc = entry;
                    // ra (when the sigaction set by user finished,if user forgets to call
                    // sys_sigreturn, it will return to sigreturn_trampoline, which
                    // calls sys_sigreturn)
                    cx.user_x[1] = sigreturn_trampoline as usize;
                    // sp (it will be used later by sys_sigreturn to restore ucontext)
                    cx.user_x[2] = ucontext_ptr;
                    task.set_sig_ucontext_ptr(ucontext_ptr);
                }
            }
        }
        Ok(())
    })
}

/// ignore the signal
fn ignore(sig: Sig) {
    log::debug!("Recevie signal {}. Action: ignore", sig);
}

/// terminate the process
fn terminate(sig: Sig) {
    log::info!("Recevie signal {}. Action: terminate", sig);
    // exit all the memers of a thread group
    let task = current_task();
    task.with_thread_group(|tg| {
        for t in tg.iter() {
            t.set_zombie();
        }
    })
}

/// stop the process
fn stop(sig: Sig) {
    log::info!("Recevie signal {}. Action: stop", sig);
    // unimplemented!()
}

/// continue the process if it is currently stopped
fn cont(sig: Sig) {
    log::info!("Recevie signal {}. Action: continue", sig);
    unimplemented!()
}

fn save_context_into_sigstack(old_blocked: SigSet) -> SysResult<usize> {
    let task = current_task();
    let cx = task.trap_context_mut();
    cx.user_fx.encounter_signal();
    let signal_stack = task.signal_stack().take();
    let stack_top = match signal_stack {
        Some(s) => s.get_stack_top(),
        None => {
            log::info!("[sigstack] signal stack use user stack");
            // 如果进程未定义专门的信号栈，用户自定义的信号处理函数将使用进程的普通栈空间，
            // 即和其他普通函数相同的栈。这个栈通常就是进程的主栈，
            // 也就是在进程启动时由操作系统自动分配的栈。
            cx.user_x[2]
        }
    };
    // extend the signal_stack
    // 在栈上压入一个UContext，存储trap frame里的寄存器信息
    let ucontext_sz = Layout::new::<UContext>().pad_to_align().size();
    let ucontext_ptr = UserWritePtr::<UContext>::from(stack_top - ucontext_sz);
    // TODO: should increase the size of the signal_stack? It seams umi doesn't do
    // that
    let ucontext = UContext {
        uc_link: 0,
        uc_sigmask: old_blocked,
        uc_stack: signal_stack.unwrap_or_default(),
        uc_mcontext: MContext {
            sepc: cx.sepc,
            user_x: cx.user_x,
        },
    };
    let ptr = ucontext_ptr.as_usize();
    log::trace!("[save_context_into_sigstack] ucontext_ptr: {ucontext_ptr:?}");
    ucontext_ptr.write(&task, ucontext)?;
    Ok(ptr)
}

pub struct WaitHandlableSignal(pub Arc<Task>);

impl Future for WaitHandlableSignal {
    type Output = usize;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        self.0
            .with_mut_sig_pending(|pending| -> Poll<Self::Output> {
                match pending.has_signal_to_handle(*current_task().sig_mask()) {
                    true => Poll::Ready(get_time_ms()),
                    false => Poll::Pending,
                }
            })
    }
}

/// A process has only one of each of the three types of timers.
pub struct ITimer {
    interval: Duration,
    next_expire: Duration,
    now: fn() -> Duration,
    activated: bool,
    sig: Sig,
}

impl ITimer {
    // pub const ITIMER_REAL: i32 = 0;
    // pub const ITIMER_VIRTUAL: i32 = 1;
    // pub const ITIMER_PROF: i32 = 2;

    /// This timer counts down in real (i.e., wall clock) time.  At each
    /// expiration, a SIGALRM signal is generated.
    pub fn new_real() -> Self {
        Self {
            interval: Duration::ZERO,
            next_expire: Duration::ZERO,
            now: get_time_duration,
            activated: false,
            sig: Sig::SIGALRM,
        }
    }

    /// This timer counts down against the user-mode CPU time consumed by the
    /// process.  (The measurement includes CPU time  consumed by all threads in
    /// the process.)  At each expiration, a SIGVTALRM signal is generated.
    pub fn new_virtual() -> Self {
        Self {
            interval: Duration::ZERO,
            next_expire: Duration::ZERO,
            now: || current_task().get_process_utime(),
            activated: false,
            sig: Sig::SIGVTALRM,
        }
    }

    /// This  timer  counts down against the total (i.e., both user and system)
    /// CPU time consumed by the process.  (The measurement includes CPU time
    /// consumed by all threads in the process.)  At each expiration, a SIGPROF
    /// signal is generated.
    /// In conjunction with ITIMER_VIRTUAL, this timer
    /// can be used to profile user and system CPU time consumed by the process.
    pub fn new_prof() -> Self {
        Self {
            interval: Duration::ZERO,
            next_expire: Duration::ZERO,
            now: || current_task().get_process_cputime(),
            activated: false,
            sig: Sig::SIGPROF,
        }
    }

    pub fn update(&mut self) {
        if !self.activated {
            return;
        }
        let now = (self.now)();
        if self.next_expire <= now {
            if self.interval.is_zero() {
                self.activated = false;
            }
            self.next_expire = now + self.interval;
            current_task().receive_signal(self.sig, false);
        }
    }

    pub fn set(&mut self, new: ITimerVal) -> ITimerVal {
        debug_assert!(new.is_valid());
        let now = (self.now)();
        let old = ITimerVal {
            it_interval: self.interval.into(),
            it_value: (self.next_expire - now).into(),
        };
        self.interval = new.it_interval.into();
        self.next_expire = now + new.it_value.into();
        self.activated = new.is_activated();
        old
    }

    pub fn get(&self) -> ITimerVal {
        ITimerVal {
            it_interval: self.interval.into(),
            it_value: (self.next_expire - (self.now)()).into(),
        }
    }
}

impl Task {
    /// this function must be calld by the task which wants to modify itself
    /// because of the `current_task()`.(i.e. it can't be called when a process
    /// wants to modify other process's itimers)
    pub fn update_itimers(&self) {
        self.with_mut_itimers(|itimers| itimers.iter_mut().for_each(|itimer| itimer.update()))
    }
}
