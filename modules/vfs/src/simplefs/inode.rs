use alloc::sync::Arc;

use vfs_core::{Inode, InodeMeta, InodeMode, SuperBlock};

pub struct SimpleInode {
    meta: InodeMeta,
}

impl SimpleInode {
    pub fn new(mode: InodeMode, super_block: Arc<dyn SuperBlock>, size: usize) -> Arc<Self> {
        Arc::new(Self {
            meta: InodeMeta::new(mode, super_block, size),
        })
    }
}

impl Inode for SimpleInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> systype::SysResult<vfs_core::Stat> {
        todo!()
    }
}
