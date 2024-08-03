use alloc::sync::Arc;

use lwext4_rust::{
    bindings::{ext4_flink, O_RDONLY, SEEK_CUR, SEEK_SET},
    InodeTypes,
};
use systype::{SysError, SysResult};
use vfs_core::{Inode, InodeMeta, InodeMode, InodeType, Stat, SuperBlock};

use crate::{map_ext4_err, map_ext4_type, LwExt4Dir, LwExt4File, Mutex, Shared};

pub struct Ext4FileInode {
    meta: InodeMeta,
    pub(crate) file: Shared<LwExt4File>,
}

unsafe impl Send for Ext4FileInode {}
unsafe impl Sync for Ext4FileInode {}

impl Ext4FileInode {
    pub fn new(super_block: Arc<dyn SuperBlock>, file: LwExt4File) -> Arc<Self> {
        let mut file = file;
        let size = file.size();
        let size: usize = size.try_into().unwrap();
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

impl Inode for Ext4FileInode {
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
        self.file.lock().truncate(len as u64);
        Ok(())
    }

    fn base_get_blk_idx(&self, offset: usize) -> SysResult<usize> {
        let mut file = self.file.lock();
        let origin_offset = file.tell();
        file.seek(offset as i64, SEEK_SET);
        let blk_idx = file.file_get_blk_idx().unwrap();
        file.seek(origin_offset as i64, SEEK_SET);
        Ok(blk_idx as usize)
    }
}

pub struct Ext4DirInode {
    meta: InodeMeta,
    pub(crate) dir: Shared<LwExt4Dir>,
}

unsafe impl Send for Ext4DirInode {}
unsafe impl Sync for Ext4DirInode {}

impl Ext4DirInode {
    pub fn new(super_block: Arc<dyn SuperBlock>, dir: LwExt4Dir) -> Arc<Self> {
        let inode = Arc::new(Self {
            meta: InodeMeta::new(InodeMode::from_type(InodeType::Dir), super_block.clone(), 0),
            dir: Arc::new(Mutex::new(dir)),
        });
        inode
    }
}

impl Inode for Ext4DirInode {
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
        Err(SysError::EINVAL)
    }

    fn base_get_blk_idx(&self, offset: usize) -> SysResult<usize> {
        Err(SysError::EINVAL)
    }
}

pub struct Ext4SymLinkInode {
    meta: InodeMeta,
}

unsafe impl Send for Ext4SymLinkInode {}
unsafe impl Sync for Ext4SymLinkInode {}

impl Ext4SymLinkInode {
    pub fn new(super_block: Arc<dyn SuperBlock>) -> Arc<Self> {
        let inode = Arc::new(Self {
            meta: InodeMeta::new(
                InodeMode::from_type(InodeType::SymLink),
                super_block.clone(),
                0,
            ),
        });
        inode
    }
}

impl Inode for Ext4SymLinkInode {
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
        Err(SysError::EINVAL)
    }

    fn base_get_blk_idx(&self, offset: usize) -> SysResult<usize> {
        Err(SysError::EINVAL)
    }
}
