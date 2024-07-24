use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};
use core::mem::MaybeUninit;

use device_core::BlockDriverOps;
use spin::Once;
use systype::SysResult;

use crate::{Dentry, FileSystemType, Inode, Mutex, StatFs};

pub struct SuperBlockMeta {
    /// Block device that hold this file system.
    // TODO: dyn file for device?
    pub device: Option<Arc<dyn BlockDriverOps>>,
    /// File system type.
    pub fs_type: Weak<dyn FileSystemType>,
    /// Root dentry points to the mount point.
    pub root_dentry: Once<Arc<dyn Dentry>>,
}

impl SuperBlockMeta {
    pub fn new(device: Option<Arc<dyn BlockDriverOps>>, fs_type: Arc<dyn FileSystemType>) -> Self {
        Self {
            device,
            root_dentry: Once::new(),
            fs_type: Arc::downgrade(&fs_type),
        }
    }
}

pub trait SuperBlock: Send + Sync {
    /// Get metadata of this super block.
    fn meta(&self) -> &SuperBlockMeta;

    /// Get filesystem statistics.
    fn stat_fs(&self) -> SysResult<StatFs>;

    /// Called when VFS is writing out all dirty data associated with a
    /// superblock.
    fn sync_fs(&self, wait: isize) -> SysResult<()>;

    fn set_root_dentry(&self, root_dentry: Arc<dyn Dentry>) {
        self.meta().root_dentry.call_once(|| root_dentry);
    }
}

impl dyn SuperBlock {
    /// Get the file system type of this super block.
    pub fn fs_type(&self) -> Arc<dyn FileSystemType> {
        self.meta().fs_type.upgrade().unwrap()
    }

    /// Get the root dentry.
    pub fn root_dentry(&self) -> Arc<dyn Dentry> {
        self.meta().root_dentry.get().unwrap().clone()
    }

    pub fn device(&self) -> Arc<dyn BlockDriverOps> {
        self.meta().device.as_ref().cloned().unwrap()
    }
}

impl<T: Send + Sync + 'static> SuperBlock for MaybeUninit<T> {
    fn meta(&self) -> &SuperBlockMeta {
        todo!()
    }

    fn stat_fs(&self) -> SysResult<StatFs> {
        todo!()
    }

    fn sync_fs(&self, _wait: isize) -> SysResult<()> {
        todo!()
    }
}
