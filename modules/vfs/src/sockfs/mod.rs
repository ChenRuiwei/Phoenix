use alloc::sync::Arc;

use device_core::BlockDriverOps;
use systype::SysResult;
use vfs_core::*;

use crate::simplefs::{dentry::SimpleDentry, inode::SimpleInode};

/// 参考https://zhuanlan.zhihu.com/p/497849394 【Linux内核 | socket底层的来龙去脉】
pub struct SockFsType {
    meta: FileSystemTypeMeta,
}

impl SockFsType {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: FileSystemTypeMeta::new("sockfs"),
        })
    }
}

impl FileSystemType for SockFsType {
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
        let sb = SockSuperBlock::new(dev, self.clone());
        let mount_dentry = SimpleDentry::new(name, sb.clone(), parent.clone());
        // SockFs的第一个Inode是DIR类型的
        let mount_inode = SimpleInode::new(InodeMode::DIR, sb.clone(), 0);
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

pub struct SockSuperBlock {
    meta: SuperBlockMeta,
}

impl SockSuperBlock {
    pub fn new(
        device: Option<Arc<dyn BlockDriverOps>>,
        fs_type: Arc<dyn FileSystemType>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: SuperBlockMeta::new(device, fs_type),
        })
    }
}

impl SuperBlock for SockSuperBlock {
    fn meta(&self) -> &SuperBlockMeta {
        &self.meta
    }

    fn stat_fs(&self) -> SysResult<StatFs> {
        // 应该是没有这个方法的？因为不涉及磁盘存储？
        todo!()
    }

    fn sync_fs(&self, wait: isize) -> SysResult<()> {
        todo!()
    }
}
