use alloc::{
    boxed::Box,
    sync::{Arc, Weak},
};
use core::{
    future::{Future, Pending},
    intrinsics::size_of,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use arch::time::get_time_duration;
use signal::*;
use systype::SysResult;
use time::timeval::{ITimerVal, TimeVal};
use timer::{Timer, TimerEvent};

use super::Task;
use crate::{mm::UserWritePtr, processor::hart::current_task_ref, task::task::TaskState};

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
        log::info!(
            "[Task::recv] tid {} recv {si:?} {:?}",
            self.tid(),
            self.with_sig_handlers(|h| h.get(si.sig))
        );
        self.with_mut_sig_pending(|pending| {
            pending.add(si);
            if pending.should_wake.contain_signal(si.sig) && self.is_interruptable() {
                log::info!("[Task::recv] tid {} has been woken", self.tid());
                self.wake();
            } else {
                log::info!(
                    "[Task::recv] tid {} hasn't been woken, should_wake {:?}, state {:?}",
                    self.tid(),
                    pending.should_wake,
                    self.state()
                );
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
            log::error!("send sigchld to parent called wait4 will cause bug now");
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
    fn _sigreturn_trampoline();
}

/// Signal dispositions and actions are process-wide: if an unhandled signal is
/// delivered to a thread, then it will affect (terminate, stop, continue, be
/// ignored in) all members of the thread group.
pub fn do_signal(task: &Arc<Task>, mut intr: bool) -> SysResult<()> {
    let old_mask = *task.sig_mask();
    let cx = task.trap_context_mut();

    while let Some(si) = task.with_mut_sig_pending(|pending| pending.dequeue_signal(&old_mask)) {
        let action = task.with_sig_handlers(|handlers| handlers.get(si.sig));
        log::info!("[do signal] Handlering signal: {:?} {:?}", si, action);
        if intr && action.flags.contains(SigActionFlag::SA_RESTART) {
            cx.sepc -= 4;
            cx.restore_last_user_a0();
            log::info!("[do_signal] restart syscall");
            intr = false;
        }
        match action.atype {
            ActionType::Ignore => {}
            ActionType::Kill => terminate(task, si.sig),
            ActionType::Stop => stop(task, si.sig),
            ActionType::Cont => cont(task, si.sig),
            ActionType::User { entry } => {
                // The signal being delivered is also added to the signal mask, unless
                // SA_NODEFER was specified when registering the handler.
                if !action.flags.contains(SigActionFlag::SA_NODEFER) {
                    task.sig_mask().add_signal(si.sig)
                };
                // 信号定义中可能包含了在处理该信号时需要阻塞的其他信号集。
                // 这些信息定义在Action的mask字段
                *task.sig_mask() |= action.mask;
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
                let mut new_sp = sp - size_of::<UContext>();
                let ucontext_ptr: UserWritePtr<UContext> = new_sp.into();
                // TODO: should increase the size of the signal_stack? It seams umi doesn't do
                // that
                let mut ucontext = UContext {
                    uc_flags: 0,
                    uc_link: 0,
                    uc_sigmask: old_mask,
                    uc_stack: signal_stack.unwrap_or_default(),
                    uc_mcontext: MContext {
                        user_x: cx.user_x,
                        fpstate: [0; 66],
                    },
                };
                ucontext.uc_mcontext.user_x[0] = cx.sepc;
                log::trace!("[save_context_into_sigstack] ucontext_ptr: {ucontext_ptr:?}");
                ucontext_ptr.write(&task, ucontext)?;
                task.set_sig_ucontext_ptr(new_sp);
                // user defined void (*sa_handler)(int);
                cx.user_x[10] = si.sig.raw();
                // if sa_flags contains SA_SIGINFO, It means user defined function is
                // void (*sa_sigaction)(int, siginfo_t *, void *ucontext); which two more
                // parameters
                // FIXME: `SigInfo` and `UContext` may not be the exact struct in C, which will
                // cause a random bug that sometimes user will trap into kernel because of
                // accessing kernel addrress
                if action.flags.contains(SigActionFlag::SA_SIGINFO) {
                    // log::error!("[SA_SIGINFO] set ucontext {ucontext:?}");
                    // a2
                    cx.user_x[12] = new_sp;
                    #[derive(Default, Copy, Clone)]
                    #[repr(C)]
                    pub struct LinuxSigInfo {
                        pub si_signo: i32,
                        pub si_errno: i32,
                        pub si_code: i32,
                        pub _pad: [i32; 29],
                        _align: [u64; 0],
                    }
                    let mut siginfo_v = LinuxSigInfo::default();
                    siginfo_v.si_signo = si.sig.raw() as _;
                    siginfo_v.si_code = si.code;
                    new_sp -= size_of::<LinuxSigInfo>();
                    let siginfo_ptr: UserWritePtr<LinuxSigInfo> = new_sp.into();
                    siginfo_ptr.write(&task, siginfo_v)?;
                    cx.user_x[11] = new_sp;
                }
                cx.sepc = entry;
                // ra (when the sigaction set by user finished,it will return to
                // _sigreturn_trampoline, which calls sys_sigreturn)
                cx.user_x[1] = _sigreturn_trampoline as usize;
                // sp (it will be used later by sys_sigreturn to restore ucontext)
                cx.user_x[2] = new_sp;
                cx.user_x[4] = ucontext.uc_mcontext.user_x[4];
                cx.user_x[3] = ucontext.uc_mcontext.user_x[3];
                // log::error!("{:#x}", new_sp);
                break;
            }
        }
    }
    Ok(())
}

/// terminate the process
fn terminate(task: &Arc<Task>, sig: Sig) {
    // exit all the memers of a thread group
    task.with_thread_group(|tg| {
        for t in tg.iter() {
            t.set_zombie();
        }
    });
    // 将信号放入低7位 (第8位是core dump标志,在gdb调试崩溃程序中用到)
    task.set_exit_code(sig.raw() as i32 & 0x7F);
}
fn stop(task: &Arc<Task>, sig: Sig) {
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
fn cont(task: &Arc<Task>, sig: Sig) {
    log::warn!("[do_signal] task continue");
    task.with_mut_thread_group(|tg| {
        for t in tg.iter() {
            t.set_running();
            t.wake();
        }
    });
    task.notify_parent(SigInfo::CLD_CONTINUED, sig);
}

#[derive(Debug)]
pub struct ITimer {
    pub interval: Duration,
    pub next_expire: Duration,
    pub enabled: bool,
}

impl ITimer {
    pub const ZERO: Self = Self {
        interval: Duration::ZERO,
        next_expire: Duration::ZERO,
        enabled: false,
    };
}

#[derive(Default, Debug)]
pub struct RealITimer {
    /// 设置Weak是因为可能task已经释放了但是RealITimer依然存在于TimerManager的情况
    pub task: Weak<Task>,
}

impl TimerEvent for RealITimer {
    // TODO: not sure
    fn callback(self: Box<Self>) -> Option<Timer> {
        if let Some(task) = self.task.upgrade() {
            task.receive_siginfo(
                SigInfo {
                    sig: Sig::SIGALRM,
                    code: SigInfo::KERNEL,
                    details: SigDetails::None,
                },
                false,
            );
            return task.with_mut_itimers(|itimers| {
                let mut real = &mut itimers[0];
                if real.interval == Duration::ZERO {
                    real.enabled = false;
                    return None;
                }
                real.next_expire = get_time_duration() + real.interval;
                Some(Timer {
                    expire: real.next_expire,
                    data: self,
                })
            });
        }

        None
    }
}

pub struct IntrBySignalFuture {
    pub task: Arc<Task>,
    pub mask: SigSet,
}

impl Future for IntrBySignalFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let has_signal = self
            .task
            .with_sig_pending(|pending| pending.has_expect_signals(!self.mask));
        if has_signal {
            log::warn!("[IntrBySignalFuture] received interupt signal");
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
