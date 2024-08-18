use alloc::{boxed::Box, sync::Arc};

use async_trait::async_trait;
use config::board::BLOCK_SIZE;
use page::Page;
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::{
    Dentry, DentryMeta, DirEntry, File, FileMeta, Inode, InodeMeta, InodeMode, Stat, SuperBlock,
};

pub struct ZeroDentry {
    meta: DentryMeta,
}

impl ZeroDentry {
    pub fn new(
        name: &str,
        super_block: Arc<dyn SuperBlock>,
        parent: Option<Arc<dyn Dentry>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new(name, super_block, parent),
        })
    }
}

impl Dentry for ZeroDentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(Arc::new(ZeroFile {
            meta: FileMeta::new(self.clone(), self.inode()?),
        }))
    }

    fn base_lookup(self: Arc<Self>, _name: &str) -> SysResult<Arc<dyn Dentry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_create(self: Arc<Self>, _name: &str, _mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_unlink(self: Arc<Self>, _name: &str) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }
}

pub struct ZeroInode {
    meta: InodeMeta,
}

impl ZeroInode {
    pub fn new(super_block: Arc<dyn SuperBlock>) -> Arc<Self> {
        let size = BLOCK_SIZE;
        Arc::new(Self {
            meta: InodeMeta::new(InodeMode::CHAR, super_block, size),
        })
    }
}

impl Inode for ZeroInode {
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

pub struct ZeroFile {
    meta: FileMeta,
}

#[async_trait]
impl File for ZeroFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, _offset: usize, buf: &mut [u8]) -> SyscallResult {
        buf.fill(0);
        Ok(buf.len())
    }

    async fn base_write_at(&self, _offset: usize, buf: &[u8]) -> SyscallResult {
        Ok(buf.len())
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }

    async fn get_page_at(&self, offset_aligned: usize) -> SysResult<Option<Arc<Page>>> {
        let page = Page::new();
        page.fill_zero();
        Ok(Some(page))
    }
}
