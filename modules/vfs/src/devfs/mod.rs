use alloc::sync::Arc;

use device_core::BlockDriverOps;
use systype::SysResult;
use vfs_core::{Dentry, FileSystemType, FileSystemTypeMeta, InodeMode, SuperBlock, SuperBlockMeta};

use self::{
    cpu_dma_latency::{CpuDmaLatencyDentry, CpuDmaLatencyInode},
    null::{NullDentry, NullInode},
    rtc::{RtcDentry, RtcInode},
    tty::{TtyDentry, TtyFile, TtyInode, TTY},
    urandom::{UrandomDentry, UrandomInode},
    zero::{ZeroDentry, ZeroInode},
};
use crate::simplefs::{dentry::SimpleDentry, inode::SimpleDirInode};

mod cpu_dma_latency;
mod null;
mod rtc;
pub mod tty;
mod urandom;
mod zero;

pub fn init_devfs(root_dentry: Arc<dyn Dentry>) -> SysResult<()> {
    let sb = root_dentry.super_block();

    let zero_dentry = ZeroDentry::new("zero", sb.clone(), Some(root_dentry.clone()));
    root_dentry.insert(zero_dentry.clone());
    let zero_inode = ZeroInode::new(sb.clone());
    zero_dentry.set_inode(zero_inode);

    let null_dentry = NullDentry::new("null", sb.clone(), Some(root_dentry.clone()));
    root_dentry.insert(null_dentry.clone());
    let null_inode = NullInode::new(sb.clone());
    null_dentry.set_inode(null_inode);

    let rtc_dentry = RtcDentry::new("rtc", sb.clone(), Some(root_dentry.clone()));
    root_dentry.insert(rtc_dentry.clone());
    let rtc_inode = RtcInode::new(sb.clone());
    rtc_dentry.set_inode(rtc_inode);

    let cpu_dma_latency_dentry =
        CpuDmaLatencyDentry::new("cpu_dma_latency", sb.clone(), Some(root_dentry.clone()));
    root_dentry.insert(cpu_dma_latency_dentry.clone());
    let cpu_dma_latency_inode = CpuDmaLatencyInode::new(sb.clone());
    cpu_dma_latency_dentry.set_inode(cpu_dma_latency_inode);

    let urandom_dentry = UrandomDentry::new("urandom", sb.clone(), Some(root_dentry.clone()));
    root_dentry.insert(urandom_dentry.clone());
    let urandom_inode = UrandomInode::new(sb.clone());
    urandom_dentry.set_inode(urandom_inode);

    let tty_dentry = TtyDentry::new("tty", sb.clone(), Some(root_dentry.clone()));
    root_dentry.insert(tty_dentry.clone());
    let tty_inode = TtyInode::new(sb.clone());
    tty_dentry.set_inode(tty_inode);
    let tty_file = TtyFile::new(tty_dentry.clone(), tty_dentry.inode()?);
    TTY.call_once(|| tty_file);

    // TODO: POSIX shm operations are not implemented yet. The code below is work
    // around to pass libc test pthread_cancel_points.
    let shm_dentry = SimpleDentry::new("shm", sb.clone(), Some(root_dentry.clone()));
    root_dentry.insert(shm_dentry.clone());
    let shm_inode = SimpleDirInode::new(InodeMode::DIR, sb.clone(), 0);
    shm_dentry.set_inode(shm_inode);

    Ok(())
}

pub struct DevFsType {
    meta: FileSystemTypeMeta,
}

impl DevFsType {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: FileSystemTypeMeta::new("devfs"),
        })
    }
}

impl FileSystemType for DevFsType {
    fn meta(&self) -> &FileSystemTypeMeta {
        &self.meta
    }

    fn base_mount(
        self: alloc::sync::Arc<Self>,
        name: &str,
        parent: Option<Arc<dyn Dentry>>,
        _flags: vfs_core::MountFlags,
        dev: Option<alloc::sync::Arc<dyn BlockDriverOps>>,
    ) -> systype::SysResult<alloc::sync::Arc<dyn vfs_core::Dentry>> {
        let sb = DevSuperBlock::new(dev, self.clone());
        let mount_dentry = SimpleDentry::new(name, sb.clone(), parent.clone());
        let mount_inode = SimpleDirInode::new(InodeMode::DIR, sb.clone(), 0);
        mount_dentry.set_inode(mount_inode.clone());
        if let Some(parent) = parent {
            parent.insert(mount_dentry.clone());
        }
        self.insert_sb(&mount_dentry.path(), sb);
        Ok(mount_dentry)
    }

    fn kill_sb(&self, _sb: alloc::sync::Arc<dyn vfs_core::SuperBlock>) -> systype::SysResult<()> {
        todo!()
    }
}

struct DevSuperBlock {
    meta: SuperBlockMeta,
}

impl DevSuperBlock {
    pub fn new(
        device: Option<Arc<dyn BlockDriverOps>>,
        fs_type: Arc<dyn FileSystemType>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: SuperBlockMeta::new(device, fs_type),
        })
    }
}

impl SuperBlock for DevSuperBlock {
    fn meta(&self) -> &SuperBlockMeta {
        &self.meta
    }

    fn stat_fs(&self) -> systype::SysResult<vfs_core::StatFs> {
        todo!()
    }

    fn sync_fs(&self, _wait: isize) -> systype::SysResult<()> {
        todo!()
    }
}
