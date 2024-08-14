#![no_std]
#![no_main]

use alloc::{ffi::CString, string::String, sync::Arc, vec};

use lwext4_rust::lwext4_readlink;
pub(crate) use lwext4_rust::{Ext4Dir as LwExt4Dir, Ext4File as LwExt4File, InodeTypes};
use sync::mutex::SpinNoIrqLock;
use systype::{SysError, SysResult};
use vfs_core::{Inode, InodeType};

extern crate alloc;

mod dentry;
mod disk;
mod file;
mod fs;
mod inode;

pub use dentry::*;
pub use file::*;
pub use fs::*;
pub use inode::*;

type Mutex<T> = SpinNoIrqLock<T>;
type Shared<T> = Arc<Mutex<T>>;

fn new_shared<T>(val: T) -> Shared<T> {
    Arc::new(Mutex::new(val))
}

fn map_ext4_err(err: i32) -> SysError {
    match err {
        2 => SysError::ENOENT,
        _ => SysError::EIO,
    }
}

pub fn map_ext4_type(value: InodeTypes) -> InodeType {
    match value {
        InodeTypes::EXT4_DE_REG_FILE => InodeType::File,
        InodeTypes::EXT4_DE_DIR => InodeType::Dir,
        InodeTypes::EXT4_DE_SYMLINK => InodeType::SymLink,
        other => unimplemented!("{:?}", other),
    }
}

pub(crate) fn readlink(path: &str) -> SysResult<CString> {
    let mut path_buf = vec![0; 512];
    let len = lwext4_readlink(path, &mut path_buf).map_err(SysError::from_i32)?;
    path_buf.truncate(len + 1);
    CString::from_vec_with_nul(path_buf).map_err(|_| SysError::EINVAL)
}
