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

pub fn spawn_timer_tasks<F>(f: F, interval_secs: usize)
where
    F: FnOnce() + Send + Copy + 'static,
{
    task::spawn_kernel_task(async move {
        let f = f;
        loop {
            f();
            ksleep_s(interval_secs).await;
        }
    });
}

pub fn print_proc_tree() {
    fn dfs_print(proc: Arc<Task>, level: usize, prefix: &str) {
        let indent = " ".repeat(level * 4);
        println!("{}{}{}", indent, prefix, proc.args_ref().join(" "));
        for (i, child) in proc.children().iter() {
            dfs_print(child.clone(), level + 1, &format!("P{i} -- "));
        }
    }

    let init = TASK_MANAGER.init_proc();
    dfs_print(init, 0, "P1 -- ");
}
