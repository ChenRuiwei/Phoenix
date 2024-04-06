pub mod aux;
mod manager;
mod pid;
mod schedule;
pub mod task;

pub use schedule::{spawn_kernel_task, spawn_user_task};
pub use task::Task;

use crate::{loader::get_app_data_by_name, mm::memory_space};

pub fn add_init_proc() {
    let elf_data = get_app_data_by_name("hello_world").unwrap();

    let _init_proc = Task::from_elf(elf_data);
}
