use alloc::sync::Arc;

use fatfs::{Read, Seek, Write};
use vfs_core::{File, FileMeta, Inode};

use crate::{as_sys_err, FatFile, Shared};

pub struct FatFileFile {
    meta: FileMeta,
    file: Shared<FatFile>,
}

impl FatFileFile {
    pub fn new(inode: Arc<dyn Inode>, file: Shared<FatFile>) -> Arc<Self> {
        todo!()
    }
}

impl File for FatFileFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> systype::SysResult<usize> {
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

    fn write(&self, offset: usize, buf: &[u8]) -> systype::SysResult<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
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

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }

    fn open(&self, inode: Arc<dyn Inode>) -> systype::SysResult<Arc<dyn File>> {
        todo!()
    }
}
