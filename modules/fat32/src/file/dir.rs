use alloc::{boxed::Box, sync::Arc};

use async_trait::async_trait;
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::{DirEntry, File, FileMeta, Inode, SeekFrom};

use crate::{
    dentry::FatDentry,
    inode::{dir::FatDirInode, FatFileInode},
    new_shared, FatDir, FatDirIter, Shared,
};

pub struct FatDirFile {
    meta: FileMeta,
    dir: Shared<FatDir>,
    iter_cache: Shared<FatDirIter>,
}

impl FatDirFile {
    pub fn new(dentry: Arc<FatDentry>, inode: Arc<FatDirInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone(), inode.clone()),
            dir: inode.dir.clone(),
            iter_cache: new_shared(inode.dir.lock().iter()),
        })
    }
}

#[async_trait]
impl File for FatDirFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, _offset: usize, _buf: &mut [u8]) -> SyscallResult {
        Err(SysError::EISDIR)
    }

    async fn base_write_at(&self, _offset: usize, _buf: &[u8]) -> SyscallResult {
        Err(SysError::EISDIR)
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }

    fn base_read_dir(&self) -> systype::SysResult<Option<vfs_core::DirEntry>> {
        let entry = self.iter_cache.lock().next();
        let Some(entry) = entry else {
            return Ok(None);
        };
        let Ok(entry) = entry else {
            return Err(SysError::EIO);
        };
        let name = entry.file_name();
        self.seek(SeekFrom::Current(1))?;
        let sub_dentry = self.dentry().get_child_or_create(&name);
        let new_inode: Arc<dyn Inode> = if entry.is_dir() {
            let new_dir = entry.to_dir();
            FatDirInode::new(self.super_block(), new_dir)
        } else {
            let new_file = entry.to_file();
            FatFileInode::new(self.super_block(), new_file)
        };
        let itype = new_inode.itype();
        sub_dentry.set_inode(new_inode);
        let entry = DirEntry {
            ino: 1,                 // Fat32 does not support ino on disk
            off: self.pos() as u64, // off should not be used
            itype,
            name,
        };
        Ok(Some(entry))
    }

    fn base_load_dir(&self) -> SysResult<()> {
        let mut iter = self.dir.lock().iter();
        while let Some(entry) = iter.next() {
            let Ok(entry) = entry else {
                return Err(SysError::EIO);
            };
            let name = entry.file_name();
            let sub_dentry = self.dentry().get_child_or_create(&name);
            let new_inode: Arc<dyn Inode> = if entry.is_dir() {
                let new_dir = entry.to_dir();
                FatDirInode::new(self.super_block(), new_dir)
            } else {
                let new_file = entry.to_file();
                FatFileInode::new(self.super_block(), new_file)
            };
            sub_dentry.set_inode(new_inode);
        }
        Ok(())
    }
}
