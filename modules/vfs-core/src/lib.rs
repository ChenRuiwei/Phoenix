#![no_std]
#![no_main]
#![feature(new_uninit)]

mod address_space;
mod dcache;
mod dentry;
mod file;
mod file_system_type;
mod inode;
mod page;
mod path;
mod super_block;
mod utils;
mod buffer_cache;

#[macro_use]
extern crate bitflags;
extern crate alloc;

pub const PERMISSION_LEN: usize = 9;

use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

pub use address_space::*;
pub use dcache::*;
pub use dentry::*;
pub use file::*;
pub use file_system_type::*;
pub use inode::*;
pub use page::*;
pub use path::*;
pub use super_block::*;
use sync::mutex::SpinNoIrqLock;
pub use utils::*;

type Mutex<T> = SpinNoIrqLock<T>;

static INODE_NUMBER: AtomicUsize = AtomicUsize::new(0);

fn alloc_ino() -> usize {
    INODE_NUMBER.fetch_add(1, Ordering::Relaxed)
}

pub fn arc_zero() -> Arc<core::mem::MaybeUninit<usize>> {
    Arc::<usize>::new_zeroed()
}
