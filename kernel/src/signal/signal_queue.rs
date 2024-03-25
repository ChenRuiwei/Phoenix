use alloc::collections::VecDeque;

use super::{KSigAction, SigHandlerManager, SigSet, SIGKILL, SIGSTOP};
use crate::{signal::SIGSEGV, stack_trace};

pub struct PendingSigs {
    sigs: VecDeque<usize>,
    bitmap: SigSet,
}

impl PendingSigs {
    fn new() -> Self {
        stack_trace!();
        Self {
            sigs: VecDeque::new(),
            bitmap: SigSet::empty(),
        }
    }

    pub fn add(&mut self, signo: usize) {
        stack_trace!();
        if !self.bitmap.contain_sig(signo) {
            self.bitmap.add_sig(signo);
            self.sigs.push_back(signo);
        }
    }

    fn pop(&mut self) -> Option<usize> {
        stack_trace!();
        if let Some(sig) = self.sigs.pop_front() {
            self.bitmap.remove_sig(sig);
            Some(sig)
        } else {
            None
        }
    }

    #[allow(unused)]
    fn contains(&self, signo: usize) -> bool {
        stack_trace!();
        self.bitmap.contain_sig(signo)
    }

    #[allow(unused)]
    pub fn is_empty(&self) -> bool {
        stack_trace!();
        self.sigs.is_empty()
    }
}

pub struct SigQueue {
    /// Pending sigs
    pub pending_sigs: PendingSigs,
    /// Blocked sigs
    pub blocked_sigs: SigSet,
    /// Signal handlers for every signal
    pub sig_handlers: SigHandlerManager,
}

impl SigQueue {
    pub fn new() -> Self {
        stack_trace!();
        Self {
            pending_sigs: PendingSigs::new(),
            blocked_sigs: SigSet::empty(),
            sig_handlers: SigHandlerManager::new(),
        }
    }
    pub fn from_another(sig_queue: &SigQueue) -> Self {
        stack_trace!();
        Self {
            pending_sigs: PendingSigs::new(),
            blocked_sigs: SigSet::empty(),
            sig_handlers: sig_queue.sig_handlers,
        }
    }
    pub fn recv_signal(&mut self, signo: usize) {
        stack_trace!();
        self.pending_sigs.add(signo);
    }

    /// Return (signo, sig action, old blocked sigs)
    pub fn check_signal(&mut self) -> Option<(usize, KSigAction, SigSet)> {
        stack_trace!();
        if self.pending_sigs.sigs.is_empty() {
            return None;
        }
        // TODO: refactor sig queue to be a bit map,
        // in order to avoid repeated signo.
        let total_len = self.pending_sigs.sigs.len();
        let mut cnt = 0;
        while cnt < total_len {
            let signo = self.pending_sigs.pop().unwrap();
            cnt += 1;
            if signo != SIGKILL && signo != SIGSTOP && self.blocked_sigs.contain_sig(signo) {
                if signo == SIGSEGV {
                    // TODO: just work around for libc-bench
                    log::warn!("SIGSEGV has been blocked");
                } else {
                    log::info!("[check_signal] sig {} has been blocked", signo);
                    self.pending_sigs.add(signo);
                    continue;
                }
            }
            log::info!("[check_signal] find a unblocked signal {}", signo);

            let old_blocked_sigs = self.blocked_sigs;

            // save_context_for_sig_handler(proc.pending_sigs.blocked_sigs);

            if self.sig_handlers.sigactions[signo].is_user_defined {
                self.blocked_sigs.add_sig(signo);
                // TODO: only use the first element now
                self.blocked_sigs |= self.sig_handlers.sigactions[signo].sig_action.sa_mask[0];
            }

            return Some((signo, self.sig_handlers.sigactions[signo], old_blocked_sigs));
        }

        None
    }
}