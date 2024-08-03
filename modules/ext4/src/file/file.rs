use alloc::{
    boxed::Box,
    ffi::CString,
    string::String,
    sync::{self, Arc},
    vec::Vec,
};
use core::{cmp, iter::zip};

use async_trait::async_trait;
use lwext4_rust::{
    bindings::{O_RDONLY, O_RDWR, SEEK_SET},
    lwext4_readlink, InodeTypes,
};
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::{DirEntry, File, FileMeta, Inode, InodeType, OpenFlags};

use crate::{
    dentry::Ext4Dentry, inode::Ext4FileInode, map_ext4_type, Ext4DirInode, Ext4LinkInode,
    LwExt4Dir, LwExt4File, Shared,
};

pub struct Ext4FileFile {
    meta: FileMeta,
    file: Shared<LwExt4File>,
}

unsafe impl Send for Ext4FileFile {}
unsafe impl Sync for Ext4FileFile {}

impl Ext4FileFile {
    pub fn new(dentry: Arc<Ext4Dentry>, inode: Arc<Ext4FileInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone(), inode.clone()),
            file: inode.file.clone(),
        })
    }
}

#[async_trait]
impl File for Ext4FileFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        match self.itype() {
            InodeType::File => {
                let mut file = self.file.lock();
                file.seek(offset as i64, SEEK_SET)
                    .map_err(SysError::from_i32)?;
                file.read(buf).map_err(SysError::from_i32)
            }
            _ => unreachable!(),
        }
    }

    async fn base_write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        match self.itype() {
            InodeType::File => {
                let mut file = self.file.lock();
                file.seek(offset as i64, SEEK_SET)
                    .map_err(SysError::from_i32)?;
                file.write(buf).map_err(SysError::from_i32)
            }
            _ => unreachable!(),
        }
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    /// Load all dentry and inodes in a directory. Will not advance dir offset.
    fn base_load_dir(&self) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }
}
