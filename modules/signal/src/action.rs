extern crate alloc;
use alloc::collections::VecDeque;
use core::{panic, task::Waker};

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

#[derive(Copy, Clone, Debug)]
pub struct Action {
    pub atype: ActionType,
    // 一个位掩码，每个比特位对应于系统中的一个信号。它用于在处理程序例程执行期间阻塞其他信号。
    // 在例程结束后，内核会重置其值，回复到信号处理之前的原值
    pub flags: SigActionFlag,
    pub mask: SigSet,
}

bitflags! {
    #[derive(Default, Copy, Clone, Debug)]
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
/// `SigPending` stores the signals received. When a task receives a signal,
/// even if the signal is blocked, it still needs to be added to the pending
/// queue. This is because the task will continue to handle the signal once it
/// is unblocked.
pub struct SigPending {
    /// 接收到的所有信号
    pub queue: VecDeque<SigInfo>,
    /// 比特位的内容代表是否收到信号，主要用来防止queue收到重复信号
    pub bitmap: SigSet,
    /// 如果在receive_siginfo的时候收到的信号位于should_wake信号集合中，
    /// 且task的wake存在，那么唤醒task
    pub should_wake: SigSet,
}

impl SigPending {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            bitmap: SigSet::empty(),
            should_wake: SigSet::empty(),
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

    /// Dequeue a signal and return the SigInfo to the caller
    pub fn dequeue_signal(&mut self, mask: &SigSet) -> Option<SigInfo> {
        let mut x = self.bitmap & (!*mask);
        let mut sig = Sig::from_i32(0);
        if !x.is_empty() {
            if !(x & SigSet::SYNCHRONOUS_MASK).is_empty() {
                x &= SigSet::SYNCHRONOUS_MASK;
            }
            sig = Sig::from_i32((x.bits().trailing_zeros() + 1) as _);
        }
        if sig.raw() == 0 {
            return None;
        }
        for i in 0..self.queue.len() {
            if self.queue[i].sig == sig {
                self.bitmap.remove_signal(sig);
                return self.queue.remove(i);
            }
        }
        log::error!("[dequeue_signal] I suppose it won't go here");
        return None;
    }

    /// Dequeue a sepcific signal in `expect` even if it is blocked and return
    /// the SigInfo to the caller
    pub fn dequeue_expect(&mut self, expect: SigSet) -> Option<SigInfo> {
        let x = self.bitmap & expect;
        if x.is_empty() {
            return None;
        }
        for i in 0..self.queue.len() {
            let sig = self.queue[i].sig;
            if x.contain_signal(sig) {
                self.bitmap.remove_signal(sig);
                return self.queue.remove(i);
            }
        }
        log::error!("[dequeue_expect] I suppose it won't go here");
        None
    }

    pub fn has_expect_signals(&self, expect: SigSet) -> bool {
        !(expect & self.bitmap).is_empty()
    }

    // #[inline]
    // pub fn has_expect_sigset(&self, expect: SigSet) -> Option<SigInfo> {
    //     let x = self.bitmap & expect;
    //     if !x.is_empty() {
    //         Some(
    //             self.queue
    //                 .iter()
    //                 .find(|si| x.contain_signal(si.sig))
    //                 .unwrap()
    //                 // TODO: NOT CLONE
    //                 .clone(),
    //         )
    //     } else {
    //         None
    //     }
    // }
}
#[derive(Clone)]
pub struct SigHandlers {
    /// 注意信号编号与数组索引有1个offset，因此在Sig中有个index()函数负责-1
    actions: [Action; NSIG],
    /// 一个位掩码，如果为1表示该信号是用户定义的，如果为0表示默认。
    /// (实际上可以由actions间接得出来，这里只是存了一个快速路径)
    bitmap: SigSet,
}

impl SigHandlers {
    pub fn new() -> Self {
        Self {
            actions: core::array::from_fn(|signo| Action::new((signo + 1).into())),
            bitmap: SigSet::empty(),
        }
    }

    pub fn get(&self, sig: Sig) -> Action {
        debug_assert!(sig.is_valid());
        self.actions[sig.index()]
    }

    pub fn update(&mut self, sig: Sig, new: Action) {
        debug_assert!(!sig.is_kill_or_stop());
        self.actions[sig.index()] = new;
        match new.atype {
            ActionType::User { .. } => self.bitmap.add_signal(sig),
            _ => self.bitmap.remove_signal(sig),
        }
    }

    /// it is used in execve because it changed the memory
    pub fn reset_user_defined(&mut self) {
        for n in 0..NSIG {
            match self.actions[n].atype {
                ActionType::User { .. } => {
                    self.actions[n].atype = ActionType::default(Sig::from_i32((n + 1) as _));
                }
                _ => {}
            }
        }
        self.bitmap = SigSet::empty();
    }

    pub fn bitmap(&self) -> SigSet {
        self.bitmap
    }
}
