use alloc::{rc::Weak, sync::Arc};

use vfs_core::{Inode, InodeMeta, InodeMode, SuperBlock};

use crate::FatDir;

pub struct FatDirInode {
    meta: InodeMeta,
    dir: FatDir,
}

impl FatDirInode {
    pub fn new(super_block: Arc<dyn SuperBlock>, dir: FatDir) -> Arc<Self> {
        // TODO: Dir size is zero?
        let inode = Arc::new(Self {
            meta: InodeMeta::new(InodeMode::Dir, &super_block, 0),
            dir,
        });
        super_block.push_inode(inode.clone());
        inode
    }
}

impl Inode for FatDirInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn create(&self, _name: &str) -> systype::SysResult<alloc::sync::Arc<dyn Inode>> {
        todo!()
    }

    fn open(&self) -> systype::SysResult<alloc::sync::Arc<dyn vfs_core::File>> {
        todo!()
    }

    fn lookup(&self, name: &str) -> systype::SysResult<alloc::sync::Arc<dyn Inode>> {
        todo!()
    }

    fn get_attr(&self) -> systype::SysResult<vfs_core::Stat> {
        todo!()
    }
}
