use alloc::sync::Arc;

use systype::{SysError, SysResult};
use vfs_core::{Inode, InodeMeta, InodeMode, SuperBlock};

use crate::{file::FatFileFile, FatFile, Mutex, Shared};

pub struct FatFileInode {
    meta: InodeMeta,
    file: Shared<FatFile>,
}

impl FatFileInode {
    pub fn new(super_block: Arc<dyn SuperBlock>, file: FatFile) -> Arc<Self> {
        let size = file.size().unwrap().try_into().unwrap();
        let inode = Arc::new(Self {
            meta: InodeMeta::new(InodeMode::File, super_block.clone(), size),
            file: Arc::new(Mutex::new(file)),
        });
        super_block.push_inode(inode.clone());
        inode
    }
}

impl Inode for FatFileInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn create(&self, dentry: Arc<dyn vfs_core::Dentry>, mode: InodeMode) -> systype::SysResult<()> {
        Err(SysError::EIO)
    }

    fn lookup(&self, dentry: Arc<dyn vfs_core::Dentry>) -> systype::SysResult<()> {
        Err(SysError::EIO)
    }

    fn get_attr(&self) -> systype::SysResult<vfs_core::Stat> {
        todo!()
    }
}
