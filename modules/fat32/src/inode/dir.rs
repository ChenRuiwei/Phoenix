use alloc::sync::Arc;

use vfs_core::{Inode, InodeMeta, InodeMode, InodeType, SuperBlock};

use crate::{new_shared, FatDir, Shared};

pub struct FatDirInode {
    meta: InodeMeta,
    pub dir: Shared<FatDir>,
}

impl FatDirInode {
    pub fn new(super_block: Arc<dyn SuperBlock>, dir: FatDir) -> Arc<Self> {
        // TODO: Dir size is zero?
        let inode = Arc::new(Self {
            meta: InodeMeta::new(InodeMode::from_type(InodeType::Dir), super_block.clone(), 0),
            dir: new_shared(dir),
        });
        super_block.push_inode(inode.clone());
        inode
    }
}

impl Inode for FatDirInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> systype::SysResult<vfs_core::Stat> {
        todo!()
    }
}
