use alloc::{boxed::Box, sync::Arc};

use async_trait::async_trait;
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::{Dentry, DirEntry, File, FileMeta, Inode};

pub struct SimpleDirFile {
    meta: FileMeta,
}

impl SimpleDirFile {
    pub fn new(dentry: Arc<dyn Dentry>, inode: Arc<dyn Inode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry, inode),
        })
    }
}

#[async_trait]
impl File for SimpleDirFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, _offset: usize, _buf: &mut [u8]) -> SyscallResult {
        Err(SysError::EISDIR)
    }

    async fn base_write(&self, _offset: usize, _buf: &[u8]) -> SyscallResult {
        Err(SysError::EISDIR)
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        todo!()
    }

    fn base_load_dir(&self) -> SysResult<()> {
        Ok(())
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }
}
