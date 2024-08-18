use alloc::sync::Arc;

use vfs_core::{Inode, InodeMeta, InodeMode, InodeType, Stat, SuperBlock};

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
        inode
    }
}

impl Inode for FatFileInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> systype::SysResult<Stat> {
        let inner = self.meta.inner.lock();
        let len = inner.size;
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: inner.mode.bits(),
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: len as u64,
            st_blksize: 512,
            __pad2: 0,
            st_blocks: (len / 512) as u64,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }
}
