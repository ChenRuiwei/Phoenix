extern crate alloc;
use alloc::collections::VecDeque;

use super::default::*;
use crate::sigset::{Sig, SigSet, NSIG};

pub enum SigActionType {
    Ignore,
    Kill,
    Suspend,
    Resume,
    User,
}
pub struct SigAction {
    pub sa_type: SigActionType,
    // 一个位掩码，每个比特位对应于系统中的一个信号。它用于在处理程序例程执行期间阻塞其他信号。
    // 在例程结束后，内核会重置其值，回复到信号处理之前的原值
    pub sa_flags: u32,
    pub sa_mask: SigSet,
}

impl SigAction {
    pub fn new(sig: Sig) -> Self {
        let sa_type = match sig {
            Sig::SIGCHLD | Sig::SIGURG => SigActionType::Ignore,
            Sig::SIGSTOP => SigActionType::Suspend,
            Sig::SIGCONT => SigActionType::Resume,
            _ => SigActionType::Kill,
        };
        Self {
            sa_type,
            sa_flags: 0,
            sa_mask: SigSet::empty(),
        }
    }
}

struct KSigAction {
    pub sa: SigAction,
}

impl KSigAction {
    pub fn new(sig: Sig) -> Self {
        Self {
            sa: SigAction::new(sig),
        }
    }
}

// 存储着进程接收到的信号队列,当进程接收到一个信号时，就需要把接收到的信号添加
// pending 这个队列中
struct SigPending {
    queue: VecDeque<Sig>,
    /// 比特位的内容代表是否收到信号
    bitmap: SigSet,
}

impl SigPending {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            bitmap: SigSet::empty(),
        }
    }

    fn add(&mut self, sig: Sig) {
        if !self.bitmap.contain_signal(sig) {
            self.queue.push_back(sig);
            self.bitmap.add_signal(sig);
        }
    }

    fn pop(&mut self, sig: Sig) -> Option<Sig> {
        if let Some(sig) = self.queue.pop_front() {
            self.bitmap.remove_signal(sig);
            Some(sig)
        } else {
            None
        }
    }

    fn contain(&self, sig: Sig) -> bool {
        self.bitmap.contain_signal(sig)
    }
}

pub struct Signal {
    /// blocked 表示被屏蔽的信息，每个位代表一个被屏蔽的信号
    blocked: SigSet,
    /// 是一个函数指针数组，代表处理动作
    handler: [KSigAction; NSIG],
    pending: SigPending,
}

impl Signal {
    pub fn new() -> Self {
        Self {
            blocked: SigSet::empty(),
            handler: core::array::from_fn(|signo| KSigAction::new(signo.into())),
            pending: SigPending::new(),
        }
    }
}
