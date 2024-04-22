use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};

use driver::BlockDevice;
use systype::SysResult;

use crate::{Dentry, FileSystemType, Inode, Mutex, StatFs};

pub struct SuperBlockMeta {
    /// Block device that hold this file system.
    // TODO: dyn file for device?
    pub device: Arc<dyn BlockDevice>,
    /// Size of a block in bytes.
    pub block_size: usize,
    /// File system type.
    pub fs_type: Weak<dyn FileSystemType>,
    /// File system statistics.
    pub stat_fs: StatFs,
    /// Root dentry points to the mount point.
    pub root_dentry: Arc<dyn Dentry>,

    /// All inodes.
    pub inodes: Mutex<Vec<Arc<dyn Inode>>>,
    /// All dirty inodes.
    pub dirty: Mutex<Vec<Arc<dyn Inode>>>,
}

pub trait SuperBlock: Send + Sync {
    /// Get metadata of this super block.
    fn meta(&self) -> &SuperBlockMeta;

    /// Set metedata of this super block.
    fn set_meta(&mut self, meta: SuperBlockMeta);

    /// Get filesystem statistics.
    fn fs_stat(&self) -> SysResult<StatFs>;

    /// Called when VFS is writing out all dirty data associated with a
    /// superblock.
    fn sync_fs(&self, wait: isize) -> SysResult<()>;
}

impl dyn SuperBlock {
    /// Get the file system type of this super block.
    fn fs_type(&self) -> Arc<dyn FileSystemType> {
        self.meta().fs_type.upgrade().unwrap()
    }

    /// Get the root dentry.
    fn root_dentry(&self) -> Arc<dyn Dentry> {
        self.meta().root_dentry.clone()
    }
}
