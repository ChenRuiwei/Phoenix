use alloc::sync::Arc;

use vfs_core::{Inode, InodeMeta, InodeMode, InodeType, SuperBlock};

use crate::{FatFile, Mutex, Shared};

pub struct FatFileInode {
    meta: InodeMeta,
    pub file: Shared<FatFile>,
}

impl FatFileInode {
    pub fn new(super_block: Arc<dyn SuperBlock>, file: FatFile) -> Arc<Self> {
        let size = file.size().unwrap().try_into().unwrap();
        let inode = Arc::new(Self {
            meta: InodeMeta::new(
                InodeMode::from_type(InodeType::File),
                super_block.clone(),
                size,
            ),
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

    fn get_attr(&self) -> systype::SysResult<vfs_core::Stat> {
        todo!()
    }
}
