#![no_std]
#![no_main]

pub mod dentry;
pub mod file;
pub mod file_system;
pub mod inode;
pub mod stat;
pub mod super_block;
pub mod utils;

#[macro_use]
extern crate bitflags;
extern crate alloc;

#[derive(Debug, Clone, Copy)]
pub enum FileSystemType {
    TmpFS,
    Fat32,
}

// 权限长度
pub const PERMISSION_LEN: usize = 9;
