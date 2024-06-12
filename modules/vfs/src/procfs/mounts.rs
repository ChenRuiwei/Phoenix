use alloc::{
    boxed::Box,
    string::{String, ToString},
    sync::Arc,
};
use core::cmp;

use async_trait::async_trait;
use config::board::BLOCK_SIZE;
use log::debug;
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::{
    Dentry, DentryMeta, DirEntry, File, FileMeta, Inode, InodeMeta, InodeMode, Stat, SuperBlock,
};

use crate::{Mutex, FS_MANAGER};

pub struct MountsDentry {
    meta: DentryMeta,
}

impl MountsDentry {
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

impl Dentry for MountsDentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(Arc::new(MountsFile {
            meta: FileMeta::new(self.clone(), self.inode()?),
        }))
    }

    fn base_lookup(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_create(self: Arc<Self>, name: &str, mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_remove(self: Arc<Self>, name: &str) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }
}

pub struct MountsInode {
    meta: InodeMeta,
}

impl MountsInode {
    pub fn new(super_block: Arc<dyn SuperBlock>, size: usize) -> Arc<Self> {
        let size = BLOCK_SIZE;
        Arc::new(Self {
            meta: InodeMeta::new(InodeMode::FILE, super_block, size),
        })
    }
}

impl Inode for MountsInode {
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

pub fn list_mounts() -> String {
    let mut res = "".to_string();
    let fs_mgr = FS_MANAGER.lock();
    for (fstype, fs) in fs_mgr.iter() {
        let supers = fs.meta().supers.lock();
        for (mount_path, sb) in supers.iter() {
            res += "proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0\n"
            // // res += fs_ptr.meta().dev_name.as_str();
            // res += " ";
            // res += mount_path.as_str();
            // res += " ";
            // res += fstype.to_string().as_str();
            // res += " ";
            // // res += fs_ptr.metadata().flags.to_string().as_str();
            // res += " 0 0\n";
        }
    }
    log::error!("{res}");
    res
}

pub struct MountsFile {
    meta: FileMeta,
}

#[async_trait]
impl File for MountsFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        log::error!("fuck");
        let info = list_mounts();
        let len = info.len();
        if self.pos() >= len {
            return Ok(0);
        }
        buf[..len].copy_from_slice(info.as_bytes());
        Ok(len)
    }

    async fn base_write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        Err(SysError::EACCES)
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }
}
