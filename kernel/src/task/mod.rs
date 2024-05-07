pub mod aux;
mod manager;
pub mod resource;
mod schedule;
pub mod signal;
pub mod task;
mod tid;

use async_utils::block_on;
pub use manager::TASK_MANAGER;
pub use schedule::{spawn_kernel_task, spawn_user_task};
pub use task::Task;
pub use tid::{PGid, Pid, Tid};
use vfs::{DISK_FS_NAME, FS_MANAGER};

use crate::{loader::get_app_data_by_name, syscall::resolve_path};

pub fn add_init_proc() {
    let elf_data = get_app_data_by_name("preliminary_tests").unwrap();
    // let elf_data = get_app_data_by_name("exec_test").unwrap();
    Task::spawn_from_elf(elf_data);
}
