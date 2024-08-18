use alloc::{boxed::Box, ffi::CString, sync::Arc};

use async_trait::async_trait;
use config::board::BLOCK_SIZE;
use crate_interface::call_interface;
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::{
    Dentry, DentryMeta, DirEntry, File, FileMeta, Inode, InodeMeta, InodeMode, Stat, SuperBlock,
};

#[crate_interface::def_interface]
pub trait KernelProcIf {
    fn exe() -> alloc::string::String;
}

pub struct ExeDentry {
    meta: DentryMeta,
}

impl ExeDentry {
    pub fn new(super_block: Arc<dyn SuperBlock>, parent: Option<Arc<dyn Dentry>>) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new("exe", super_block, parent),
        })
    }
}

impl Dentry for ExeDentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(Arc::new(ExeFile {
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

pub struct ExeInode {
    meta: InodeMeta,
}

impl ExeInode {
    pub fn new(super_block: Arc<dyn SuperBlock>, _size: usize) -> Arc<Self> {
        let size = BLOCK_SIZE;
        Arc::new(Self {
            meta: InodeMeta::new(InodeMode::LINK, super_block, size),
        })
    }
}

impl Inode for ExeInode {
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

pub struct ExeFile {
    meta: FileMeta,
}

#[async_trait]
impl File for ExeFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, _offset: usize, buf: &mut [u8]) -> SyscallResult {
        todo!()
    }

    async fn base_write_at(&self, _offset: usize, _buf: &[u8]) -> SyscallResult {
        Err(SysError::EACCES)
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }

    async fn readlink(&self, buf: &mut [u8]) -> SyscallResult {
        let exe = call_interface!(KernelProcIf::exe());
        if buf.len() < exe.len() + 1 {
            log::warn!("readlink buf not big enough");
            return Err(SysError::EINVAL);
        }
        buf[0..exe.len()].copy_from_slice(exe.as_bytes());
        buf[exe.len()] = '\0' as u8;
        Ok(exe.len() + 1)
    }
}
