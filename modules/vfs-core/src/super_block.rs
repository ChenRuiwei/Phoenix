use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};

use driver::BlockDevice;
use spin::Once;
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
    /// Root dentry points to the mount point.
    pub root_dentry: Once<Arc<dyn Dentry>>,

    /// All inodes.
    pub inodes: Mutex<Vec<Arc<dyn Inode>>>,
    /// All dirty inodes.
    pub dirty: Mutex<Vec<Arc<dyn Inode>>>,
}

impl SuperBlockMeta {
    pub fn new(device: Arc<dyn BlockDevice>, fs_type: Arc<dyn FileSystemType>) -> Self {
        let block_size = device.block_size();
        Self {
            device,
            block_size,
            root_dentry: Once::new(),
            fs_type: Arc::downgrade(&fs_type),
            inodes: Mutex::new(Vec::new()),
            dirty: Mutex::new(Vec::new()),
        }
    }
}

pub trait SuperBlock: Send + Sync {
    /// Get metadata of this super block.
    fn meta(&self) -> &SuperBlockMeta;

    /// Get filesystem statistics.
    fn fs_stat(&self) -> SysResult<StatFs>;

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

    pub fn push_inode(&self, inode: Arc<dyn Inode>) {
        self.meta().inodes.lock().push(inode)
    }
}
