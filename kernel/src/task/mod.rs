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
    // let elf_data = get_app_data_by_name("futex_test").unwrap();
    Task::spawn_from_elf(elf_data);
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
                    // TODO: let logging more specific
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
