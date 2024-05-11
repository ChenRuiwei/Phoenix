use alloc::{
    boxed::Box,
    ffi::CString,
    string::{String, ToString},
    sync::Arc,
};
use core::sync::atomic::Ordering;

use async_trait::async_trait;
use fatfs::{Read, Seek, Write};
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::{Dentry, DirEntry, File, FileMeta, Inode, InodeMode, InodeType, SeekFrom};

use crate::{
    as_sys_err,
    dentry::{self, FatDentry},
    inode::{self, dir::FatDirInode, FatFileInode},
    new_shared, DiskCursor, FatDir, FatDirIter, Shared,
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

    async fn base_read(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        Err(SysError::EISDIR)
    }

    async fn base_write(&self, offset: usize, buf: &[u8]) -> SyscallResult {
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
        let pos = self.pos();
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
        let inode = self
            .inode()
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;
        let mut iter = inode.dir.lock().iter();
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

    /// Called when the VFS needs to move the file position index.
    ///
    /// Return the result offset.
    fn seek(&self, pos: SeekFrom) -> SysResult<usize> {
        let mut res_pos = self.pos();
        match pos {
            SeekFrom::Current(off) => match off {
                1 => res_pos += off as usize,
                -1 => {
                    res_pos -= off.abs() as usize;
                    let mut iter = self.dir.lock().iter();
                    iter.nth(res_pos);
                    *self.iter_cache.lock() = iter;
                }
                _ => unimplemented!(),
            },
            SeekFrom::Start(off) => {
                unimplemented!()
            }
            SeekFrom::End(off) => {
                unimplemented!()
            }
        }
        self.set_pos(res_pos);
        Ok(res_pos)
    }
}
