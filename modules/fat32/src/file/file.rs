use alloc::{
    boxed::Box,
    string::{String, ToString},
    sync::Arc,
};

use async_trait::async_trait;
use fatfs::{Read, Seek, Write};
use systype::{ASyscallResult, SysError, SyscallResult};
use vfs_core::{Dentry, DirEntry, File, FileMeta, Inode, InodeMode, InodeType, SeekFrom};

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

#[async_trait]
impl File for FatFileFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        match self.itype() {
            InodeType::File => {
                let mut file = self.file.lock();
                let fat_offset = file.offset() as usize;
                if offset != fat_offset {
                    file.seek(fatfs::SeekFrom::Start(offset as u64))
                        .map_err(as_sys_err)?;
                }
                let count = file.read(buf).map_err(as_sys_err)?;
                log::trace!("[FatFileFile::base_read] count {count}");
                Ok(count)
            }
            _ => Err(SysError::EISDIR),
        }
    }

    async fn base_write(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        if buf.is_empty() {
            return Ok(0);
        }
        match self.itype() {
            InodeType::File => {
                let mut file = self.file.lock();
                let size = self.inode().size();
                // TODO: should we write at offset which is bigger than size
                if offset > size {
                    // write empty data to fill area [size, offset)
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
                self.seek(SeekFrom::Start((offset + buf.len()) as u64));
                Ok(buf.len())
            }
            _ => Err(SysError::EISDIR),
        }
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }

    fn base_read_dir(&self) -> systype::SysResult<Option<vfs_core::DirEntry>> {
        todo!()
    }
}
