use alloc::sync::Arc;
use core::{
    future::{Future, Pending},
    intrinsics::size_of,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use arch::time::get_time_duration;
use signal::{
    action::{Action, ActionType, SigActionFlag},
    siginfo::{SigDetails, SigInfo},
    signal_stack::{MContext, UContext},
    sigset::{Sig, SigSet},
};
use systype::SysResult;
use time::timeval::ITimerVal;

use super::Task;
use crate::{mm::UserWritePtr, processor::hart::current_task, task::task::TaskState};

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
                        if task.sig_mask_ref().contain_signal(si.sig) {
                            continue;
                        }
                        task.recv(si);
                        signal_delivered = true;
                        break;
                    }
                    if !signal_delivered {
                        let task = tg.iter().next().unwrap();
                        task.recv(si);
                    }
                })
            }
            true => self.recv(si),
        }
    }
    fn recv(&self, si: SigInfo) {
        self.with_mut_sig_pending(|pending| {
            pending.add(si);
            if self.is_interruptable() && pending.should_wake.contain_signal(si.sig) {
                self.wake();
            }
        });
    }

    pub fn set_wake_up_signal(&self, except: SigSet) {
        debug_assert!(self.is_interruptable());
        self.with_mut_sig_pending(|pending| pending.should_wake = except)
    }

    fn notify_parent(self: &Arc<Self>, code: i32, signum: Sig) {
        let parent = self.parent().unwrap().upgrade().unwrap();
        if !parent
            .with_sig_handlers(|handlers| handlers.get(Sig::SIGCHLD))
            .flags
            .contains(SigActionFlag::SA_NOCLDSTOP)
        {
            parent.receive_siginfo(
                SigInfo {
                    sig: Sig::SIGCHLD,
                    code,
                    details: SigDetails::CHLD {
                        pid: self.pid(),
                        status: signum.raw() as i32 & 0x7F,
                        utime: self.time_stat().user_time(),
                        stime: self.time_stat().sys_time(),
                    },
                },
                false,
            );
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
    let old_mask = *task.sig_mask();
    loop {
        if let Some(si) = task.with_mut_sig_pending(|pending| pending.dequeue_signal(&old_mask)) {
            log::info!("[do signal] Handlering signal: {:?}", si);
            let action = task.with_sig_handlers(|handlers| handlers.get(si.sig));
            log::info!("[do signal] {:?}", action);
            match action.atype {
                ActionType::Ignore => {}
                ActionType::Kill => terminate(si.sig),
                ActionType::Stop => stop(si.sig, &task),
                ActionType::Cont => cont(si.sig, &task),
                ActionType::User { entry } => {
                    // The signal being delivered is also added to the signal mask, unless
                    // SA_NODEFER was specified when registering the handler.
                    if !action.flags.contains(SigActionFlag::SA_NODEFER) {
                        task.sig_mask().add_signal(si.sig)
                    };
                    // 信号定义中可能包含了在处理该信号时需要阻塞的其他信号集。
                    // 这些信息定义在Action的mask字段
                    *task.sig_mask() |= action.mask;
                    let cx = task.trap_context_mut();
                    cx.user_fx.encounter_signal();
                    let signal_stack = task.sig_stack().take();
                    let sp = match signal_stack {
                        Some(s) => {
                            log::error!("[sigstack] use user defined signal stack. Unimplemented");
                            s.get_stack_top()
                        }
                        None => {
                            // 如果进程未定义专门的信号栈，
                            // 用户自定义的信号处理函数将使用进程的普通栈空间，
                            // 即和其他普通函数相同的栈。这个栈通常就是进程的主栈，
                            // 也就是在进程启动时由操作系统自动分配的栈。
                            cx.user_x[2]
                        }
                    };
                    // extend the signal_stack
                    // 在栈上压入一个UContext，存储trap frame里的寄存器信息
                    let ucontext_ptr = UserWritePtr::<UContext>::from(sp - size_of::<UContext>());
                    // TODO: should increase the size of the signal_stack? It seams umi doesn't do
                    // that
                    let ucontext = UContext {
                        uc_link: 0,
                        uc_sigmask: old_mask,
                        uc_stack: signal_stack.unwrap_or_default(),
                        uc_mcontext: MContext {
                            sepc: cx.sepc,
                            user_x: cx.user_x,
                        },
                    };
                    log::trace!("[save_context_into_sigstack] ucontext_ptr: {ucontext_ptr:?}");
                    let mut new_sp = ucontext_ptr.as_usize();
                    ucontext_ptr.write(&task, ucontext)?;
                    // user defined void (*sa_handler)(int);
                    cx.user_x[10] = si.sig.raw();
                    // if sa_flags contains SA_SIGINFO, It means user defined function is
                    // void (*sa_sigaction)(int, siginfo_t *, void *ucontext); which two more
                    // parameters
                    if action.flags.contains(SigActionFlag::SA_SIGINFO) {
                        // a2
                        cx.user_x[12] = new_sp;
                        let siginfo_ptr =
                            UserWritePtr::<SigInfo>::from(new_sp - size_of::<SigInfo>());
                        new_sp = siginfo_ptr.as_usize();
                        siginfo_ptr.write(&task, si.clone())?;
                        cx.user_x[11] = new_sp;
                    }
                    cx.sepc = entry;
                    // ra (when the sigaction set by user finished,it will return to
                    // sigreturn_trampoline, which calls sys_sigreturn)
                    cx.user_x[1] = sigreturn_trampoline as usize;
                    // sp (it will be used later by sys_sigreturn to restore ucontext)
                    cx.user_x[2] = new_sp;
                    task.set_sig_ucontext_ptr(new_sp);
                }
            }
        } else {
            break;
        }
    }
    Ok(())
}

/// terminate the process
fn terminate(sig: Sig) {
    // exit all the memers of a thread group
    let task = current_task();
    task.with_thread_group(|tg| {
        for t in tg.iter() {
            t.set_zombie();
        }
    });
    // 将信号放入低7位 (第8位是core dump标志,在gdb调试崩溃程序中用到)
    task.set_exit_code(sig.raw() as i32 & 0x7F);
}
fn stop(sig: Sig, task: &Arc<Task>) {
    log::warn!("[do_signal] task stopped!");
    task.with_mut_thread_group(|tg| {
        for t in tg.iter() {
            t.set_stopped();
            t.set_wake_up_signal(SigSet::SIGCONT);
        }
    });
    task.notify_parent(SigInfo::CLD_STOPPED, sig);
}
/// continue the process if it is currently stopped
fn cont(sig: Sig, task: &Arc<Task>) {
    log::warn!("[do_signal] task continue");
    task.with_mut_thread_group(|tg| {
        for t in tg.iter() {
            t.set_running();
            t.wake();
        }
    });
    task.notify_parent(SigInfo::CLD_CONTINUED, sig);
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
