mod meminfo;
mod mounts;

use alloc::sync::Arc;

use device_core::BlockDriverOps;
use systype::{SysResult, SyscallResult};
use vfs_core::{
    Dentry, FileSystemType, FileSystemTypeMeta, InodeMode, MountFlags, SuperBlock, SuperBlockMeta,
};

use self::{
    meminfo::{MemInfoDentry, MemInfoInode},
    mounts::{MountsDentry, MountsInode},
};
use crate::simplefs::{
    dentry::{self, SimpleDentry},
    inode::{SimpleDirInode, SimpleFileInode},
};

pub fn init_procfs(root_dentry: Arc<dyn Dentry>) -> SysResult<()> {
    let mem_info_dentry = MemInfoDentry::new(
        "meminfo",
        root_dentry.super_block(),
        Some(root_dentry.clone()),
    );
    let mem_info_inode = MemInfoInode::new(root_dentry.super_block(), 0);
    mem_info_dentry.set_inode(mem_info_inode);
    root_dentry.insert(mem_info_dentry);

    let mounts_dentry = MountsDentry::new(
        "mounts",
        root_dentry.super_block(),
        Some(root_dentry.clone()),
    );
    let mounts_inode = MountsInode::new(root_dentry.super_block(), 0);
    mounts_dentry.set_inode(mounts_inode);
    root_dentry.insert(mounts_dentry);
    Ok(())
}

pub struct ProcFsType {
    meta: FileSystemTypeMeta,
}

impl ProcFsType {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: FileSystemTypeMeta::new("procfs"),
        })
    }
}

impl FileSystemType for ProcFsType {
    fn meta(&self) -> &FileSystemTypeMeta {
        &self.meta
    }

    fn base_mount(
        self: Arc<Self>,
        name: &str,
        parent: Option<Arc<dyn Dentry>>,
        flags: MountFlags,
        dev: Option<Arc<dyn BlockDriverOps>>,
    ) -> SysResult<Arc<dyn Dentry>> {
        let sb = ProcSuperBlock::new(dev, self.clone());
        let mount_dentry = SimpleDentry::new(name, sb.clone(), parent.clone());
        let mount_inode = SimpleDirInode::new(InodeMode::DIR, sb.clone(), 0);
        mount_dentry.set_inode(mount_inode.clone());
        if let Some(parent) = parent {
            parent.insert(mount_dentry.clone());
        }
        self.insert_sb(&mount_dentry.path(), sb);
        Ok(mount_dentry)
    }

    fn kill_sb(&self, sb: Arc<dyn SuperBlock>) -> SysResult<()> {
        todo!()
    }
}

pub struct ProcSuperBlock {
    meta: SuperBlockMeta,
}

impl ProcSuperBlock {
    pub fn new(
        device: Option<Arc<dyn BlockDriverOps>>,
        fs_type: Arc<dyn FileSystemType>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: SuperBlockMeta::new(device, fs_type),
        })
    }
}

impl SuperBlock for ProcSuperBlock {
    fn meta(&self) -> &SuperBlockMeta {
        &self.meta
    }

    fn stat_fs(&self) -> SysResult<vfs_core::StatFs> {
        todo!()
    }

    fn sync_fs(&self, wait: isize) -> SysResult<()> {
        todo!()
    }
}
