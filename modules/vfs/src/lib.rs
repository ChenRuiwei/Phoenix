#![no_std]
#![no_main]

mod dentry;
mod file;
mod file_system;
mod inode;
mod stat;
mod super_block;
mod utils;

#[macro_use]
extern crate bitflags;

pub enum FileSystemType {
    TmpFS,
    Fat32,
}

// 权限长度
pub const PERMISSION_LEN: usize = 9;
// 文件默认权限，所有用户可读，可写，不可执行
pub const DEFAULT_PERMISSION_FILE: bits = 0o666;
// 文件夹默认权限，所有者可以读、写和执行，组用户和其他用户只能读和执行
pub const DEFAULT_PERMISSION_DIR: bits = 0o755;
