use alloc::sync::Arc;

use device_core::BlockDevice;
use systype::SysResult;
use vfs_core::{
    Dentry, FileSystemType, FileSystemTypeMeta, InodeMode, MountFlags, StatFs, SuperBlock,
    SuperBlockMeta,
};

use crate::simplefs::{dentry::SimpleDentry, inode::SimpleDirInode};

pub struct TmpFsType {
    meta: FileSystemTypeMeta,
}

impl TmpFsType {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: FileSystemTypeMeta::new("tmpfs"),
        })
    }
}

impl FileSystemType for TmpFsType {
    fn meta(&self) -> &FileSystemTypeMeta {
        &self.meta
    }

    fn base_mount(
        self: Arc<Self>,
        name: &str,
        parent: Option<Arc<dyn Dentry>>,
        _flags: MountFlags,
        dev: Option<Arc<dyn BlockDevice>>,
    ) -> SysResult<Arc<dyn Dentry>> {
        let sb = TmpSuperBlock::new(dev, self.clone());
        let mount_dentry = SimpleDentry::new(name, sb.clone(), parent.clone());
        let mount_inode = SimpleDirInode::new(InodeMode::DIR, sb.clone(), 0);
        mount_dentry.set_inode(mount_inode.clone());
        if let Some(parent) = parent {
            parent.insert(mount_dentry.clone());
        }
        self.insert_sb(&mount_dentry.path(), sb);
        Ok(mount_dentry)
    }

    fn kill_sb(&self, _sb: Arc<dyn SuperBlock>) -> SysResult<()> {
        todo!()
    }
}

pub struct TmpSuperBlock {
    meta: SuperBlockMeta,
}

impl TmpSuperBlock {
    pub fn new(
        device: Option<Arc<dyn BlockDevice>>,
        fs_type: Arc<dyn FileSystemType>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: SuperBlockMeta::new(device, fs_type),
        })
    }
}

impl SuperBlock for TmpSuperBlock {
    fn meta(&self) -> &SuperBlockMeta {
        &self.meta
    }

    fn stat_fs(&self) -> SysResult<StatFs> {
        todo!()
    }

    fn sync_fs(&self, _wait: isize) -> SysResult<()> {
        todo!()
    }
}
