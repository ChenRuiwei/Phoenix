use alloc::{boxed::Box, sync::Arc};

use async_trait::async_trait;
use fatfs::{Read, Seek, Write};
use systype::{SysError, SyscallResult};
use vfs_core::{File, FileMeta, InodeType};

use crate::{as_sys_err, dentry::FatDentry, inode::file::FatFileInode, FatFile, Shared};

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

    async fn base_read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        match self.itype() {
            InodeType::File => {
                let mut file = self.file.lock();
                let fat_offset = file.offset() as usize;
                if offset != fat_offset {
                    file.seek(fatfs::SeekFrom::Start(offset as u64))
                        .map_err(as_sys_err)?;
                }
                let mut count = 0;
                let mut buf = buf;
                while !buf.is_empty() {
                    match file.read(buf).map_err(as_sys_err)? {
                        0 => break,
                        n => {
                            buf = &mut buf[n..];
                            count += n;
                        }
                    }
                }
                log::trace!("[FatFileFile::base_read] count {count}");
                Ok(count)
            }
            InodeType::Dir => Err(SysError::EISDIR),
            _ => unreachable!(),
        }
    }

    async fn base_write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        if buf.is_empty() {
            return Ok(0);
        }
        match self.itype() {
            InodeType::File => {
                let mut file = self.file.lock();
                let size = self.inode().size();
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
                Ok(buf.len())
            }
            InodeType::Dir => Err(SysError::EISDIR),
            _ => unreachable!(),
        }
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }

    fn base_read_dir(&self) -> systype::SysResult<Option<vfs_core::DirEntry>> {
        todo!()
    }
}
