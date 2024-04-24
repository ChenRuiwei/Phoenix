use alloc::{
    string::{String, ToString},
    sync::Arc,
};

use fatfs::{Read, Seek, Write};
use systype::SysError;
use vfs_core::{DirEnt, File, FileMeta, Inode, InodeMode, SeekFrom};

use crate::{
    as_sys_err,
    inode::{self, dir::FatDirInode},
    FatDir, Shared,
};

pub struct FatDirFile {
    meta: FileMeta,
    dir: Shared<FatDir>,
}

impl FatDirFile {
    pub fn new(path: String, inode: Arc<FatDirInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(path, inode.clone()),
            dir: inode.dir.clone(),
        })
    }
}

impl File for FatDirFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> systype::SysResult<usize> {
        Err(SysError::EISDIR)
    }

    fn write(&self, offset: usize, buf: &[u8]) -> systype::SysResult<usize> {
        Err(SysError::EISDIR)
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }

    fn read_dir(&self) -> systype::SysResult<Option<vfs_core::DirEnt>> {
        let inode = self
            .inode()
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::EIO)?;
        let pos = self.pos();
        let entry = inode.dir.lock().iter().nth(pos);
        if let Some(entry) = entry {
            match entry {
                Ok(entry) => {
                    self.seek(vfs_core::SeekFrom::Current(1));
                    let ty = if entry.is_dir() {
                        InodeMode::Dir
                    } else {
                        InodeMode::File
                    };
                    let entry = DirEnt {
                        ino: 1,
                        ty,
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
