use alloc::sync::Arc;

use driver::BlockDevice;
use systype::SysResult;

use crate::{dentry::Dentry, FileSystemType, FsStat};

pub struct SuperBlockMeta {
    /// Block device that hold this file system.
    pub device: Arc<dyn BlockDevice>,
    /// Size of a block in bytes.
    pub block_size: usize,
    /// File system type.
    pub fs_type: FileSystemType,
    /// File system statistics.
    pub fs_stat: FsStat,
}

pub trait SuperBlock: Send + Sync {
    /// Get metadata of this super block.
    fn meta(&self) -> &SuperBlockMeta;

    /// Set metedata of this super block.
    fn set_meta(&mut self, meta: SuperBlockMeta);

    /// Get filesystem statistics.
    fn fs_stat(&self) -> SysResult<FsStat>;

    /// Get the file system type of this super block.
    fn fs_type(&self) -> FileSystemType;

    /// Get the root dentry.
    fn root_dentry(&self) -> SysResult<Arc<dyn Dentry>>;
}
