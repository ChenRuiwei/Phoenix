use self::ctx::EnvContext;
use crate::{task::Task, trap::TrapContext};

pub mod ctx;
pub mod env;
pub mod hart;

use alloc::sync::Arc;

pub use env::SumGuard;

pub use self::hart::{local_hart, HARTS};

pub fn local_env() -> &'static mut EnvContext {
    local_hart().env_mut()
}

pub fn current_task() -> &'static Arc<Task> {
    local_hart().current_task()
}

pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task().trap_context_mut()
}
