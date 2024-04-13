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
extern crate alloc;

#[derive(Debug, Clone, Copy)]
pub enum FileSystemType {
    TmpFS,
    Fat32,
}

// 权限长度
pub const PERMISSION_LEN: usize = 9;
