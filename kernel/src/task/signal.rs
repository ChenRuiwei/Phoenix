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
    action::{Action, ActionType, SigActionFlag},
    siginfo::{SigDetails, SigInfo},
    signal_stack::{MContext, UContext},
    sigset::{Sig, SigSet},
};
use systype::SysResult;
use time::timeval::ITimerVal;

use super::Task;
use crate::{mm::UserWritePtr, processor::hart::current_task};

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct SigAction {
    /// sa_handler specifies the action to be associated with signum and can be
    /// one of the following:
    /// 1. SIG_DFL for the default action
    /// 2. SIG_IGN to ignore this signal
    /// 3. A pointer to a signal handling function. This function receives the
    ///    signal number as its only argument.
    pub sa_handler: usize,
    pub sa_flags: SigActionFlag,
    pub restorer: usize,
    /// sa_mask specifies a mask of signals which should be blocked during
    /// execution of the signal handler.
    pub sa_mask: SigSet,
}
pub const SIG_DFL: usize = 0;
pub const SIG_IGN: usize = 1;

impl From<Action> for SigAction {
    fn from(action: Action) -> Self {
        let sa_handler = match action.atype {
            ActionType::Ignore => SIG_IGN,
            ActionType::Kill | ActionType::Stop | ActionType::Cont => SIG_DFL,
            ActionType::User { entry } => entry.into(),
        };
        Self {
            sa_handler,
            sa_flags: action.flags,
            restorer: 0,
            sa_mask: action.mask,
        }
    }
}

impl Task {
    /// A signal may be process-directed or thread-directed
    /// A process-directed signal is targeted at a thread group and is
    /// delivered to an arbitrarily selected thread from among those that are
    /// not blocking the signal
    /// A thread-directed signal is targeted at
    /// (i.e., delivered to) a specific thread.
    pub fn receive_siginfo(&self, si: SigInfo, thread_directed: bool) {
        match thread_directed {
            false => {
                debug_assert!(self.is_leader());
                self.with_mut_thread_group(|tg| {
                    // The thread group will prioritize selecting a thread that has not blocked the
                    // signal. If all threads have blocked the signal, a random thread will be
                    // selected to receive the signal
                    let mut signal_delivered = false;
                    for task in tg.iter() {
                        if task.sig_mask().contain_signal(si.sig) {
                            continue;
                        }
                        task.with_mut_sig_pending(|pending| {
                            pending.recv(si);
                        });
                        signal_delivered = true;
                        break;
                    }
                    if !signal_delivered {
                        let task = tg.iter().next().unwrap();
                        task.with_mut_sig_pending(|pending| {
                            pending.recv(si);
                        });
                    }
                })
            }
            true => {
                self.with_mut_sig_pending(|pending| {
                    pending.recv(si);
                });
            }
        }
    }
}

extern "C" {
    fn sigreturn_trampoline();
}

/// Signal dispositions and actions are process-wide: if an unhandled signal is
/// delivered to a thread, then it will affect (terminate, stop, continue, be
/// ignored in) all members of the thread group.
pub fn do_signal() -> SysResult<()> {
    let task = current_task();
    task.with_mut_sig_pending(|pending| -> SysResult<()> {
        // if there is no signal to be handle, just return
        if pending.is_empty() {
            return Ok(());
        }
        log::info!("[do signal] there are some signals to be handled");
        let len = pending.queue.len();
        for _ in 0..len {
            let si = pending.pop().unwrap();
            if !si.sig.is_kill_or_stop() && task.sig_mask().contain_signal(si.sig) {
                pending.add(si);
                continue;
            }
            let action = task.with_sig_handlers(|handlers| handlers.get(si.sig).clone());
            log::debug!("[do_signal] handling signal {}", si.sig);
            match action.atype {
                ActionType::Ignore => {
                    log::debug!("Recevie signal {}. Action: ignore", si.sig);
                }
                ActionType::Kill => terminate(si.sig),
                ActionType::Stop => stop(si.sig),
                ActionType::Cont => cont(si.sig),
                ActionType::User { entry } => {
                    log::info!("[do signal] user defined signal action");
                    // 在跳转到用户定义的信号处理程序之前，内核需要保存当前进程的上下文，
                    // 包括程序计数器、寄存器状态、栈指针等。此外，当前的信号屏蔽集也需要被保存，
                    // 因为信号处理程序可能会被嵌套调用（即一个信号处理程序中可能会触发另一个信号），
                    // 所以需要确保每个信号处理程序能恢复到它被调用时的屏蔽集状态
                    let old_mask = *task.sig_mask();
                    // The signal being delivered is also added to the signal mask, unless
                    // SA_NODEFER was specified when registering the handler.
                    if !action.flags.contains(SigActionFlag::SA_NODEFER) {
                        task.sig_mask().add_signal(si.sig)
                    };
                    // 信号定义中可能包含了在处理该信号时需要阻塞的其他信号集。
                    // 这些信息定义在Action的mask字段
                    *task.sig_mask() |= action.mask;

                    let ucontext_ptr = save_context_into_sigstack(old_mask)?;
                    let cx = task.trap_context_mut();
                    // 用户自定义的sa_handler的参数，void myhandler(int signo,siginfo_t *si,void
                    // *ucontext); TODO:实现siginfo
                    // a0
                    cx.user_x[10] = si.sig.raw();
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

/// terminate the process
fn terminate(sig: Sig) {
    log::info!("Recevie signal {}. Action: terminate", sig);
    // exit all the memers of a thread group
    let task = current_task();
    task.with_thread_group(|tg| {
        for t in tg.iter() {
            t.set_zombie();
        }
    });
    task.set_exit_code(sig.raw() as i32);
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
    let signal_stack = task.sig_stack().take();
    let stack_top = match signal_stack {
        Some(s) => {
            log::info!("[sigstack] use user defined signal stack");
            s.get_stack_top()
        }
        None => {
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

/// wait for signals and don't need siginfo as return value
pub struct WaitExpectSignals<'a> {
    task: &'a Arc<Task>,
    expect: SigSet,
    set_waker: bool,
}

impl<'a> WaitExpectSignals<'a> {
    pub fn new(task: &'a Arc<Task>, expect: SigSet) -> Self {
        Self {
            task,
            expect,
            set_waker: false,
        }
    }
}

impl<'a> Future for WaitExpectSignals<'a> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if !self.set_waker {
            self.task.with_mut_sig_pending(|pending| {
                pending.set_waker(Some(cx.waker().clone()));
            });
        };
        self.task
            .with_mut_sig_pending(|pending| -> Poll<Self::Output> {
                match pending.has_expect_signals(self.expect) {
                    true => {
                        pending.set_waker(None);
                        Poll::Ready(())
                    }
                    false => Poll::Pending,
                }
            })
    }
}

/// wait for a signal and need the siginfo of the expected signal
pub struct WaitExpectSignal<'a> {
    task: &'a Arc<Task>,
    expect: Sig,
    set_waker: bool,
}

impl<'a> WaitExpectSignal<'a> {
    pub fn new(task: &'a Arc<Task>, expect: Sig) -> Self {
        Self {
            task,
            expect,
            set_waker: false,
        }
    }
}

impl<'a> Future for WaitExpectSignal<'a> {
    type Output = SigInfo;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if !self.set_waker {
            self.task.with_mut_sig_pending(|pending| {
                pending.set_waker(Some(cx.waker().clone()));
            });
            self.set_waker = true;
            return Poll::Pending;
        };
        self.task
            .with_mut_sig_pending(|pending| -> Poll<Self::Output> {
                if let Some(si) = pending.has_expect_signal(self.expect) {
                    pending.set_waker(None);
                    Poll::Ready(si)
                } else {
                    Poll::Pending
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
            current_task().receive_siginfo(
                SigInfo {
                    sig: self.sig,
                    // The SI-TIMER value indicates that the signal was triggered by a timer
                    // expiration. This usually refers to POSIX timers set through the
                    // timer_settime() function, rather than traditional UNIX timers set through
                    // setter() or alarm(). Therefore, only set si_code field SI_KERNEL
                    code: SigInfo::KERNEL,
                    details: SigDetails::None,
                },
                false,
            );
        }
    }

    // TODO: It may be something wrong?
    pub fn set(&mut self, new: ITimerVal) -> ITimerVal {
        debug_assert!(new.is_valid());
        let now = (self.now)();
        let old = ITimerVal {
            it_interval: self.interval.into(),
            it_value: if self.next_expire < now {
                Duration::ZERO.into()
            } else {
                (self.next_expire - now).into()
            },
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
