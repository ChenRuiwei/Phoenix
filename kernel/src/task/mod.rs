pub mod aux;
mod manager;
pub mod resource;
mod schedule;
pub mod signal;
pub mod task;
mod tid;

pub use manager::TASK_MANAGER;
pub use schedule::{spawn_kernel_task, spawn_user_task};
pub use task::Task;
pub use tid::{PGid, Pid, Tid};

use crate::loader::get_app_data_by_name;

pub fn add_init_proc() {
    let elf_data = get_app_data_by_name("init_proc").unwrap();
    Task::spawn_from_elf(elf_data);
}
