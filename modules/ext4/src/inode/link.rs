use alloc::sync::Arc;

use config::board::BLOCK_MASK;
use lwext4_rust::{
    bindings::{ext4_flink, O_RDONLY, SEEK_CUR, SEEK_SET},
    InodeTypes,
};
use systype::{SysError, SysResult};
use vfs_core::{Inode, InodeMeta, InodeMode, InodeType, Stat, SuperBlock};

use crate::{map_ext4_err, map_ext4_type, LwExt4Dir, LwExt4File, Mutex, Shared};

pub struct Ext4LinkInode {
    meta: InodeMeta,
}

unsafe impl Send for Ext4LinkInode {}
unsafe impl Sync for Ext4LinkInode {}

impl Ext4LinkInode {
    pub fn new(target: &str, super_block: Arc<dyn SuperBlock>) -> Arc<Self> {
        let inode = Arc::new(Self {
            meta: InodeMeta::new(
                InodeMode::from_type(InodeType::SymLink),
                super_block.clone(),
                target.len(),
            ),
        });
        inode
    }
}

impl Inode for Ext4LinkInode {
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
            st_nlink: inner.nlink as _,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: len as u64,
            st_blksize: 512,
            __pad2: 0,
            st_blocks: ((len + BLOCK_MASK) / 512) as u64,
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
