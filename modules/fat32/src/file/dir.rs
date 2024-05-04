use alloc::{
    boxed::Box,
    ffi::CString,
    string::{String, ToString},
    sync::Arc,
};

use async_trait::async_trait;
use fatfs::{Read, Seek, Write};
use systype::{SysError, SyscallResult};
use vfs_core::{Dentry, DirEntry, File, FileMeta, Inode, InodeMode, InodeType, SeekFrom};

use crate::{
    as_sys_err,
    dentry::FatDentry,
    inode::{self, dir::FatDirInode},
    FatDir, Shared,
};

pub struct FatDirFile {
    meta: FileMeta,
    dir: Shared<FatDir>,
}

impl FatDirFile {
    pub fn new(dentry: Arc<FatDentry>, inode: Arc<FatDirInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone(), inode.clone()),
            dir: inode.dir.clone(),
        })
    }
}

#[async_trait]
impl File for FatDirFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn read(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        Err(SysError::EISDIR)
    }

    async fn write(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        Err(SysError::EISDIR)
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }

    fn read_dir(&self) -> systype::SysResult<Option<vfs_core::DirEntry>> {
        let inode = self
            .inode()
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::EIO)?;
        let pos = self.pos();
        let entry = inode.dir.lock().iter().nth(pos);
        if let Some(entry) = entry {
            match entry {
                Ok(entry) => {
                    self.seek(vfs_core::SeekFrom::Current(1))?;
                    let itype = if entry.is_dir() {
                        InodeType::Dir
                    } else {
                        InodeType::File
                    };
                    let entry = DirEntry {
                        ino: 1,                 // Fat32 does not support ino on disk
                        off: self.pos() as u64, // off should not be used
                        itype,
                        name: entry.file_name(),
                    };
                    Ok(Some(entry))
                }
                Err(_) => Err(SysError::EIO),
            }
        } else {
            Ok(None)
        }
    }
}
