use alloc::{
    string::{String, ToString},
    sync::Arc,
};

use fatfs::{Read, Seek, Write};
use systype::SysError;
use vfs_core::{Dentry, DirEnt, File, FileMeta, Inode, InodeMode, InodeType, SeekFrom};

use crate::{
    as_sys_err,
    dentry::{self, FatDentry},
    inode::{self, dir::FatDirInode, file::FatFileInode},
    FatFile, Shared,
};

pub struct FatFileFile {
    meta: FileMeta,
    file: Shared<FatFile>,
}

impl FatFileFile {
    pub fn new(dentry: Arc<FatDentry>, inode: Arc<FatFileInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone(), inode.clone()),
            file: inode.file.clone(),
        })
    }
}

impl File for FatFileFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> systype::SysResult<usize> {
        match self.inode().node_type() {
            InodeType::File => {
                let mut file = self.file.lock();
                let fat_offset = file.offset() as usize;
                if offset != fat_offset {
                    file.seek(fatfs::SeekFrom::Start(offset as u64))
                        .map_err(as_sys_err)?;
                }
                let mut buf = buf;
                let mut count = 0;
                while !buf.is_empty() {
                    let len = file.read(buf).map_err(as_sys_err)?;
                    if len == 0 {
                        break;
                    }
                    count += len;
                    buf = &mut buf[len..];
                }
                Ok(count)
            }
            _ => Err(SysError::EISDIR),
        }
    }

    fn write(&self, offset: usize, buf: &[u8]) -> systype::SysResult<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        match self.inode().node_type() {
            InodeType::File => {
                let mut file = self.file.lock();
                let size = self.inode().size();
                if offset > size {
                    let empty = vec![0; offset - size];
                    file.seek(fatfs::SeekFrom::Start(size as u64))
                        .map_err(as_sys_err)?;
                    file.write_all(&empty).map_err(as_sys_err)?;
                }
                let fat_offset = file.offset() as usize;
                if offset != fat_offset {
                    file.seek(fatfs::SeekFrom::Start(offset as u64))
                        .map_err(as_sys_err)?;
                }
                file.write_all(buf).map_err(as_sys_err)?;
                if offset + buf.len() > size {
                    let new_size = offset + buf.len();
                    self.inode().set_size(new_size);
                }
                Ok(buf.len())
            }
            _ => Err(SysError::EISDIR),
        }
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }

    fn read_dir(&self) -> systype::SysResult<Option<vfs_core::DirEnt>> {
        match self.inode().node_type() {
            InodeType::Dir => {
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
                                InodeMode::DIR
                            } else {
                                InodeMode::FILE
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
            _ => Err(SysError::ENOTDIR),
        }
    }
}
