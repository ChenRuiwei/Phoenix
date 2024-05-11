extern crate alloc;
use alloc::collections::VecDeque;
use core::task::Waker;

use bitflags::*;

use crate::{
    siginfo::SigInfo,
    sigset::{Sig, SigSet, NSIG},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    Ignore,
    Kill,
    Stop,
    Cont,
    User { entry: usize },
}

impl ActionType {
    pub fn default(sig: Sig) -> Self {
        match sig {
            Sig::SIGCHLD | Sig::SIGURG | Sig::SIGWINCH => ActionType::Ignore,
            Sig::SIGSTOP | Sig::SIGTSTP | Sig::SIGTTIN | Sig::SIGTTOU => ActionType::Stop,
            Sig::SIGCONT => ActionType::Cont,
            _ => ActionType::Kill,
        }
    }
}

#[derive(Copy, Clone)]
pub struct Action {
    pub atype: ActionType,
    // 一个位掩码，每个比特位对应于系统中的一个信号。它用于在处理程序例程执行期间阻塞其他信号。
    // 在例程结束后，内核会重置其值，回复到信号处理之前的原值
    pub flags: SigActionFlag,
    pub mask: SigSet,
}

bitflags! {
    #[derive(Default, Copy, Clone)]
    pub struct SigActionFlag : usize {
        const SA_NOCLDSTOP = 1;
        const SA_NOCLDWAIT = 2;
        const SA_SIGINFO = 4;
        const SA_ONSTACK = 0x08000000;
        const SA_RESTART = 0x10000000;
        const SA_NODEFER = 0x40000000;
        const SA_RESETHAND = 0x80000000;
        const SA_RESTORER = 0x04000000;
    }
}

impl Action {
    pub fn new(sig: Sig) -> Self {
        let atype = ActionType::default(sig);
        Self {
            atype,
            flags: Default::default(),
            mask: SigSet::empty(),
        }
    }
}

/// 存储着进程接收到的信号队列,当进程接收到一个信号时，
/// 就需要把接收到的信号添加到 pending
/// 这个队列中，即使被block了，因为在被解除block时task还是会接着处理这个信号。
/// TODO:可否只留有一个bitmap?
pub struct SigPending {
    /// 接收到的所有信号
    pub queue: VecDeque<SigInfo>,
    /// 比特位的内容代表是否收到信号，主要用来防止queue收到重复信号
    pub bitmap: SigSet,
    pub waker: Option<Waker>,
}

impl SigPending {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            bitmap: SigSet::empty(),
            waker: None,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn add(&mut self, si: SigInfo) {
        if !self.bitmap.contain_signal(si.sig) {
            self.bitmap.add_signal(si.sig);
            self.queue.push_back(si);
        }
    }

    pub fn pop(&mut self) -> Option<SigInfo> {
        if let Some(si) = self.queue.pop_front() {
            self.bitmap.remove_signal(si.sig);
            Some(si)
        } else {
            None
        }
    }

    #[inline]
    pub fn has_expect_signals(&self, expect: SigSet) -> bool {
        !(expect & self.bitmap).is_empty()
    }

    #[inline]
    pub fn has_expect_signal(&self, expect: Sig) -> Option<SigInfo> {
        if self.bitmap.contain_signal(expect) {
            Some(
                self.queue
                    .iter()
                    .find(|si| si.sig == expect)
                    .unwrap()
                    .clone(),
            )
        } else {
            None
        }
    }

    #[inline]
    pub fn set_waker(&mut self, waker: Option<Waker>) {
        self.waker = waker;
    }

    pub fn recv(&mut self, si: SigInfo) {
        self.add(si);
        if let Some(waker) = self.waker.as_ref() {
            waker.wake_by_ref(); // 调用 wake_by_ref，不消耗 waker
        }
    }
}

pub struct SigHandlers {
    /// 注意信号编号与数组索引有1个offset，因此在Sig中有个index()函数负责-1
    actions: [Action; NSIG],
}

impl SigHandlers {
    pub fn new() -> Self {
        Self {
            actions: core::array::from_fn(|signo| Action::new((signo + 1).into())),
        }
    }

    pub fn get(&self, sig: Sig) -> Action {
        debug_assert!(sig.is_valid());
        self.actions[sig.index()]
    }

    /// This function will not update the default processing of SIG_KILL and
    /// SIG_STOP signals
    pub fn update(&mut self, sig: Sig, new: Action) {
        // let old = self.actions[sig.index()];
        if sig.is_kill_or_stop() {
            return;
        }
        self.actions[sig.index()] = new;
    }
}
