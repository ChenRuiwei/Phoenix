use alloc::{boxed::Box, sync::Arc, vec::Vec};

use async_trait::async_trait;
use sync::mutex::SpinNoIrqLock;
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

    async fn base_read_at(&self, _offset: usize, _buf: &mut [u8]) -> SyscallResult {
        Err(SysError::EISDIR)
    }

    async fn base_write_at(&self, _offset: usize, _buf: &[u8]) -> SyscallResult {
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

pub struct SimpleFileFile {
    meta: FileMeta,
    content: SpinNoIrqLock<Vec<u8>>,
}

impl SimpleFileFile {
    pub fn new(dentry: Arc<dyn Dentry>, inode: Arc<dyn Inode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry, inode),
            content: SpinNoIrqLock::new(Vec::new()),
        })
    }
}

#[async_trait]
impl File for SimpleFileFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        let content = self.content.lock();
        let len = core::cmp::min(buf.len(), content.len() - offset);
        buf[..len].copy_from_slice(&content[offset..offset + len]);
        Ok(len)
    }

    async fn base_write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        let mut content = self.content.lock();

        if content.len() < offset {
            content.resize(offset, 0);
        }

        // content.len() >= offset
        let in_content_len = core::cmp::min(buf.len(), content.len() - offset);
        for i in 0..in_content_len {
            content[i + offset] = buf[i];
        }

        let out_content_len = buf.len() - in_content_len;
        for i in 0..out_content_len {
            content.push(buf[i + in_content_len]);
        }

        Ok(buf.len())
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_load_dir(&self) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }
}
