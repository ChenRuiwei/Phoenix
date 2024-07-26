pub mod aux;
mod manager;
pub mod resource;
mod schedule;
pub mod signal;
pub mod task;
mod tid;

use alloc::{string::ToString, sync::Arc, vec, vec::Vec};

use async_utils::block_on;
use config::process::USER_STACK_SIZE;
pub use manager::{PROCESS_GROUP_MANAGER, TASK_MANAGER};
pub use schedule::{spawn_kernel_task, spawn_user_task};
pub use task::Task;
pub use tid::{PGid, Pid, Tid, TID_ALLOCATOR};
use vfs::sys_root_dentry;
use vfs_core::Path;

use crate::{
    mm::memory_space::{self, init_stack, MemorySpace},
    processor::env::within_sum,
    trap::TrapContext,
};

pub fn spawn_init_proc() {
    let init_proc_path = "/init_proc";
    let args = vec![init_proc_path.to_string()];
    let envp = Vec::new();

    let file = Path::new(sys_root_dentry(), sys_root_dentry(), init_proc_path)
        .walk()
        .unwrap()
        .open()
        .unwrap();
    let elf_data = block_on(async { file.read_all().await }).unwrap();

    let mut memory_space = MemorySpace::new_user();
    unsafe { memory_space.switch_page_table() };
    let (entry, auxv) = memory_space.parse_and_map_elf(file.clone(), &elf_data);
    let sp_init = memory_space.alloc_stack_lazily(USER_STACK_SIZE);
    let (sp, argc, argv, envp) = within_sum(|| init_stack(sp_init, args.clone(), envp, auxv));
    memory_space.alloc_heap_lazily();

    let trap_context = TrapContext::new(entry, sp);

    let task = Task::new_init(memory_space, trap_context, file, args);
    schedule::spawn_user_task(task);
}

#[macro_export]
macro_rules! generate_state_methods {
    ($($state:ident),+) => {
        $(
            paste::paste! {
                #[allow(unused)]
                pub fn [<is_ $state:lower>](&self) -> bool {
                    *self.state.lock() == TaskState::$state
                }
                #[allow(unused)]
                pub fn [<set_ $state:lower>](&self) {
                    *self.state.lock() = TaskState::$state
                }
            }
        )+
    };
}

#[macro_export]
macro_rules! generate_with_methods {
    ($($name:ident : $ty:ty),+) => {
        paste::paste! {
            $(
                #[allow(unused)]
                pub fn [<with_ $name>]<T>(&self, f: impl FnOnce(&$ty) -> T) -> T {
                    log::trace!("with_{}", stringify!($name));
                    f(&self.$name.lock())
                }
                #[allow(unused)]
                pub fn [<with_mut_ $name>]<T>(&self, f: impl FnOnce(&mut $ty) -> T) -> T {
                    log::trace!("with_mut_{}", stringify!($name));
                    f(&mut self.$name.lock())
                }
            )+
        }
    };
}

#[macro_export]
macro_rules! generate_accessors {
    ($($field_name:ident : $field_type:ty),+) => {
        paste::paste! {
            $(
                #[allow(unused)]
                pub fn $field_name(&self) -> &mut $field_type {
                    unsafe { &mut *self.$field_name.get() }
                }
                #[allow(unused)]
                pub fn [<$field_name _ref>](&self) -> &$field_type {
                    unsafe { &*self.$field_name.get() }
                }
            )+
        }
    };
}

#[macro_export]
macro_rules! generate_atomic_accessors {
    ($($field_name:ident : $field_type:ty),+) => {
        paste::paste! {
            $(
                #[allow(unused)]
                pub fn $field_name(&self) -> $field_type {
                    self.$field_name.load(Ordering::Relaxed)
                }
                #[allow(unused)]
                pub fn [<set_ $field_name>](&self, value: $field_type) {
                    self.$field_name.store(value, Ordering::Relaxed);
                }
            )+
        }
    };
}
