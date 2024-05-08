use alloc::sync::Arc;

use vfs_core::{Inode, InodeMeta, InodeMode, Stat, SuperBlock};

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
        let inner = self.meta.inner.lock();
        let mode = self.meta.mode.bits();
        let len = inner.size;
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: mode,
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
