use alloc::{format, sync::Arc};

use timer::timelimited_task::ksleep_s;

use crate::task::{self, Task, TASK_MANAGER};

/// Code block that only runs in debug mode.
#[macro_export]
macro_rules! when_debug {
    ($blk:expr) => {
        cfg_if::cfg_if! {
            if #[cfg(debug_assertions)] {
                $blk
            }
        }
    };
}

/// Used for debug.
pub fn exam_hash(buf: &[u8]) -> usize {
    let mut h: usize = 5381;
    for c in buf {
        h = h.wrapping_mul(33).wrapping_add(*c as usize);
    }
    h
}
