use alloc::sync::Arc;

use lwext4_rust::{
    bindings::{ext4_flink, O_RDONLY, SEEK_CUR, SEEK_SET},
    InodeTypes,
};
use systype::{SysError, SysResult};
use vfs_core::{Inode, InodeMeta, InodeMode, InodeType, Stat, SuperBlock};

use crate::{map_ext4_err, map_ext4_type, LwExt4Dir, LwExt4File, Mutex, Shared};

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
        let len = inner.size;
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: inner.mode.bits(),
            st_nlink: inner.nlink as _,
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
