pub mod aux;
mod manager;
pub mod resource;
mod schedule;
pub mod signal;
pub mod task;
mod tid;

use alloc::vec::Vec;

pub use manager::TASK_MANAGER;
pub use schedule::{spawn_kernel_task, spawn_user_task, yield_now};
pub use task::Task;
pub use tid::{PGid, Pid, Tid};
use vfs::{DISK_FS_NAME, FS_MANAGER};

use crate::loader::get_app_data_by_name;

pub fn add_init_proc() {
    let elf_data = get_app_data_by_name("exec_test").unwrap();
    // let elf_data = get_app_data_by_name("preliminary_tests").unwrap();

    // let mut buf = [0; 512];
    // let sb = FS_MANAGER
    //     .lock()
    //     .get(DISK_FS_NAME)
    //     .unwrap()
    //     .get_sb("/")
    //     .unwrap();
    //
    // let root_dentry = sb.root_dentry();
    // let mut elf_data = Vec::new();
    // let test_dentry = root_dentry.lookup("getpid").unwrap();
    // let test_file = test_dentry.open().unwrap();
    // test_file.read_all_from_start(&mut elf_data);

    Task::spawn_from_elf(&elf_data);
}
