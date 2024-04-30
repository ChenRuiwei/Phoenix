extern crate alloc;
use alloc::collections::VecDeque;

use crate::sigset::{Sig, SigSet, NSIG};

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
    pub flags: usize,
    pub mask: SigSet,
}

impl Action {
    pub fn new(sig: Sig) -> Self {
        let atype = ActionType::default(sig);
        Self {
            atype,
            flags: 0,
            mask: SigSet::empty(),
        }
    }
}

/// 存储着进程接收到的信号队列,当进程接收到一个信号时，
/// 就需要把接收到的信号添加到 pending 这个队列中。TODO:可否只留有一个bitmap?
pub struct SigPending {
    /// 接收到的所有信号
    pub queue: VecDeque<Sig>,
    /// 比特位的内容代表是否收到信号，主要用来防止queue收到重复信号
    pub bitmap: SigSet,
}

impl SigPending {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            bitmap: SigSet::empty(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn add(&mut self, sig: Sig) {
        if !self.bitmap.contain_signal(sig) {
            self.queue.push_back(sig);
            self.bitmap.add_signal(sig);
        }
    }

    pub fn pop(&mut self) -> Option<Sig> {
        if let Some(sig) = self.queue.pop_front() {
            self.bitmap.remove_signal(sig);
            Some(sig)
        } else {
            None
        }
    }

    pub fn has_signal_to_handle(&self, blocked: SigSet) -> bool {
        // if there is any signal in pending list and it haven't been blocked
        !(!blocked & self.bitmap).is_empty()
    }

    pub fn contain(&self, sig: Sig) -> bool {
        self.bitmap.contain_signal(sig)
    }
}

pub struct SigHandlers {
    /// 注意信号编号与数组索引有1个offset，因此在Sig中有个index()函数负责-1
    actions: [Action; NSIG],
}

impl SigHandlers {
    pub fn new() -> Self {
        Self {
            // TODO: 这里应该是要+1吧，因为信号从1开始
            actions: core::array::from_fn(|signo| Action::new((signo + 1).into())),
        }
    }

    // TODO: debug asssert sig is valid
    pub fn get(&self, sig: Sig) -> Option<&Action> {
        if sig.is_valid() {
            Some(&self.actions[sig.index()])
        } else {
            None
        }
    }

    /// This function will not replace the default processing of SIG_KILL and
    /// SIG_STOP signals
    pub fn replace(&mut self, sig: Sig, new: Action) -> Action {
        let old = self.actions[sig.index()];
        if sig.is_kill_or_stop() {
            return old;
        }
        self.actions[sig.index()] = new;
        old
    }
}
