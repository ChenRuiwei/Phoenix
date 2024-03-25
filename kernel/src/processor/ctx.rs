use alloc::sync::Arc;

use riscv::register::sstatus;
use sync::cell::SyncUnsafeCell;

use crate::{
    mm::PageTable, process::thread::Thread, utils::stack_trace::stack_tracker::StackTracker,
};

/// Owned by `Hart` to hold some context.
///
/// There are three types of LocalContext.
///
/// The first one is initialized in `rust_main`, and the Hart owning it will
/// keep running in `rust_main` as an executor, it is not an async task, let's
/// just call it IDLE context, this hart not only runs a executor, but
/// also handles kernel interrupts.
///
/// The second one is async kernel task spawned by `spawn_kernel_thread`. Let's
/// call it Kernel Task context.
///
/// The third one is async user task spawned by `spawn_user_thread`. Let's call
/// it User Task context.
///
/// Both the first two types are running in kernel, they have no
/// UserTaskContext. And will not trap into user.
pub struct LocalContext {
    /// Only be None if it is an executor or kernel task running
    user_task_ctx: Option<UserTaskContext>,
    env: EnvContext,
}

impl LocalContext {
    pub fn new(user_task_ctx: Option<UserTaskContext>) -> Self {
        let env = EnvContext::new();
        Self { user_task_ctx, env }
    }
    pub fn task_ctx_mut(&mut self) -> &mut UserTaskContext {
        match self.user_task_ctx.as_mut() {
            Some(user_ctx) => user_ctx,
            None => panic!("Idle LocalContext"),
        }
    }
    pub fn task_ctx(&self) -> &UserTaskContext {
        match self.user_task_ctx.as_ref() {
            Some(user_ctx) => user_ctx,
            None => panic!("Idle LocalContext"),
        }
    }
    pub fn env_mut(&mut self) -> &mut EnvContext {
        &mut self.env
    }
    pub fn env(&self) -> &EnvContext {
        &self.env
    }
    /// Whether there is no user task now(i.e. kernel thread is running)
    pub fn is_idle(&self) -> bool {
        self.user_task_ctx.is_none()
    }
}

pub struct UserTaskContext {
    pub thread: Arc<Thread>,
    /// Although we can get pagetable from the thread's process's memory space,
    /// it needs lock, which reduces performance.
    pub page_table: Arc<SyncUnsafeCell<PageTable>>,
}

/// Store some permission flags
pub struct EnvContext {
    /// Supervisor interrupt disable
    sie_disabled: usize,
    /// Permit supervisor user memory access
    sum_enabled: usize,
    /// Stack tracker
    pub stack_tracker: StackTracker,
}

impl EnvContext {
    pub fn new() -> Self {
        Self {
            sie_disabled: 0,
            sum_enabled: 0,
            stack_tracker: StackTracker::new(),
        }
    }

    // pub fn sie_dec(&mut self) {
    //     if self.sie_disabled == 0 {
    //         unsafe {
    //             sstatus::clear_sie();
    //         }
    //     }
    //     self.sie_disabled += 1;
    // }

    // pub fn sie_inc(&mut self) {
    //     if self.sie_disabled == 1 {
    //         unsafe {
    //             sstatus::set_sie();
    //         }
    //     }
    //     self.sie_disabled -= 1;
    // }

    pub fn sum_inc(&mut self) {
        if self.sum_enabled == 0 {
            unsafe {
                sstatus::set_sum();
            }
        }
        self.sum_enabled += 1
    }

    pub fn sum_dec(&mut self) {
        if self.sum_enabled == 1 {
            unsafe {
                sstatus::clear_sum();
            }
        }
        self.sum_enabled -= 1
    }

    /// Return whether the new task should open kernel interrupt or not
    pub fn env_change(new: &Self, old: &Self) -> bool {
        unsafe {
            if (new.sum_enabled > 0) != (old.sum_enabled > 0) {
                if new.sum_enabled > 0 {
                    sstatus::set_sum();
                } else {
                    sstatus::clear_sum();
                }
            }
            // if (new.sie > 0) != (old.sie > 0) {
            //     if new.sie > 0 {
            //         EnvContext::enable_sie();
            //     } else {
            //         sstatus::clear_sie();
            //     }
            // }
        }
        new.sie_disabled == 0
    }
}
