use signal::{
    action::{Action, ActionType},
    sigset::{Sig, SigSet},
};

use crate::processor::hart::current_task;

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
        let old_blocked = signal.blocked;
        match signal.handlers.get(sig).unwrap().atype {
            ActionType::Ignore => ignore(sig),
            ActionType::Kill => terminate(sig),
            ActionType::Stop => stop(sig),
            ActionType::Cont => cont(sig),
            ActionType::User { entry } => {}
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
