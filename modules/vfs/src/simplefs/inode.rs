use alloc::sync::Arc;

use page::PageCache;
use systype::SysResult;
use vfs_core::{Inode, InodeMeta, InodeMode, Stat, SuperBlock};

pub struct SimpleFileInode {
    meta: InodeMeta,
}

impl SimpleFileInode {
    pub fn new(mode: InodeMode, super_block: Arc<dyn SuperBlock>, size: usize) -> Arc<Self> {
        debug_assert!(mode.to_type().is_file());
        let mut meta =  InodeMeta::new(mode, super_block, size);
        meta.page_cache = Some(PageCache::new());
        Arc::new(Self {
            meta,
        })
    }
}

impl Inode for SimpleFileInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
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

    fn base_truncate(&self, len: usize) -> SysResult<()> {
        Ok(())
    }
}

pub struct SimpleDirInode {
    meta: InodeMeta,
}

impl SimpleDirInode {
    pub fn new(mode: InodeMode, super_block: Arc<dyn SuperBlock>, size: usize) -> Arc<Self> {
        debug_assert!(mode.to_type().is_dir());
        Arc::new(Self {
            meta: InodeMeta::new(mode, super_block, size),
        })
    }
}

impl Inode for SimpleDirInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
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
