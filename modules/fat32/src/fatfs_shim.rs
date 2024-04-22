use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::cmp::min;

use driver::BlockDevice;
use fatfs::{Dir, File, LossyOemCpConverter, NullTimeProvider, Read, Seek, SeekFrom, Write};
use log::debug;
use sync::mutex::SpinNoIrqLock;
use systype::{SysError, SysResult};
use vfs::Inode;

use crate::FatFs;

pub trait DiskOperation {
    fn read_block(index: usize, buf: &mut [u8]);
    fn write_block(index: usize, data: &[u8]);
}

type Mutex<T> = SpinNoIrqLock<T>;

pub struct Fat32FileSystem {
    inner: FatFs,
}

unsafe impl Send for Fat32FileSystem {}
unsafe impl Sync for Fat32FileSystem {}

impl FileSystem for Fat32FileSystem {
    fn fs_name(&self) -> String {
        "fat32".to_string()
    }
    fn root_dir(&'static self) -> Arc<dyn Inode> {
        Arc::new(FatDir {
            filename: String::from(""),
            inner: self.inner.root_dir(),
        })
    }
}

impl Fat32FileSystem {
    pub fn new(blk_dev: Arc<dyn BlockDevice>) -> Arc<Self> {
        let cursor: DiskCursor = DiskCursor {
            sector: 0,
            offset: 0,
            blk_dev,
        };
        let inner = fatfs::FileSystem::new(cursor, fatfs::FsOptions::new()).expect("open fs wrong");
        Arc::new(Self { inner })
    }
}

pub struct FatFileInner {
    inner: File<'static, DiskCursor, NullTimeProvider, LossyOemCpConverter>,
    size: usize,
}

pub struct FatFile {
    filename: String,
    inner: Mutex<FatFileInner>,
}

unsafe impl Sync for FatFile {}
unsafe impl Send for FatFile {}

pub struct FatDir {
    filename: String,
    inner: Dir<'static, DiskCursor, NullTimeProvider, LossyOemCpConverter>,
}

unsafe impl Sync for FatDir {}
unsafe impl Send for FatDir {}

impl vfs::File for FatFile {
    fn read(&self, offset: usize, buffer: &mut [u8]) -> SysResult<usize> {
        let mut inner = self.inner.lock();

        if offset >= inner.size {
            return Ok(0);
        }
        let seek_curr = SeekFrom::Start(offset as _);
        inner.inner.seek(seek_curr).map_err(as_sys_err)?;
        let len = inner.size;
        debug!("off: {:#x} rlen: {:#x}", offset, len);
        // read cached file.
        inner
            .inner
            .seek(SeekFrom::Start(offset as u64))
            .map_err(as_sys_err)?;
        let rlen = min(buffer.len(), len as usize - offset);
        inner
            .inner
            .read_exact(&mut buffer[..rlen])
            .map_err(as_sys_err)?;
        Ok(rlen)
    }

    fn write(&self, offset: usize, buffer: &[u8]) -> SysResult<usize> {
        let mut inner = self.inner.lock();

        // if offset > len
        let seek_curr = SeekFrom::Start(offset as _);
        let curr_off = inner.inner.seek(seek_curr).map_err(as_sys_err)? as usize;
        if offset != curr_off {
            let buffer = vec![0u8; 512];
            loop {
                let wlen = min(offset - inner.size, 512);

                if wlen == 0 {
                    break;
                }
                let real_wlen = inner.inner.write(&buffer).map_err(as_sys_err)?;
                inner.size += real_wlen;
            }
        }

        inner.inner.write_all(buffer).map_err(as_sys_err)?;

        if offset + buffer.len() > inner.size {
            inner.size = offset + buffer.len();
        }
        Ok(buffer.len())
    }

    fn flush(&self) -> SysResult<()> {
        Ok(())
    }

    fn fsync(&self) -> SysResult<()> {
        Ok(())
    }
}

impl Inode for FatFile {
    fn inode_type(&self) -> vfs::mode {
        todo!()
    }

    fn node_perm(&self) -> vfs::NodePermission {
        vfs::NodePermission::empty()
    }

    fn create(
        &self,
        _name: &str,
        _ty: vfs::mode,
        _perm: vfs::NodePermission,
        _rdev: Option<u64>,
    ) -> SysResult<Arc<dyn Inode>> {
        Err(SysError::ENOSYS)
    }

    fn link(&self, _name: &str, _src: Arc<dyn Inode>) -> SysResult<Arc<dyn Inode>> {
        Err(SysError::ENOSYS)
    }

    fn unlink(&self, _name: &str) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    fn symlink(&self, _name: &str, _sy_name: &str) -> SysResult<Arc<dyn Inode>> {
        Err(SysError::ENOSYS)
    }

    fn lookup(&self, _name: &str) -> SysResult<Arc<dyn Inode>> {
        Err(SysError::ENOSYS)
    }

    fn rmdir(&self, _name: &str) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    fn readlink(&self, _buf: &mut [u8]) -> SysResult<usize> {
        Err(SysError::ENOSYS)
    }

    fn set_attr(&self, _attr: vfs::InodeAttr) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    fn get_attr(&self) -> SysResult<vfs::FileStat> {
        Err(SysError::ENOSYS)
    }

    fn list_xattr(&self) -> SysResult<Vec<String>> {
        Err(SysError::ENOSYS)
    }

    fn truncate(&self, _len: u64) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    fn rename_to(
        &self,
        _old_name: &str,
        _new_parent: Arc<dyn Inode>,
        _new_name: &str,
        _flag: vfs::RenameFlag,
    ) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    fn update_time(&self, _time: vfs::Time, _now: vfs::TimeSpec) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }
}

impl vfs::File for FatDir {
    fn read(&self, _offset: usize, _buf: &mut [u8]) -> SysResult<usize> {
        Err(SysError::ENOSYS)
    }

    fn write(&self, _offset: usize, _buf: &[u8]) -> SysResult<usize> {
        Err(SysError::ENOSYS)
    }

    fn read_dir(&self, _start_index: usize) -> SysResult<Option<vfs::DirEntry>> {
        Err(SysError::ENOSYS)
    }

    fn ioctl(&self, _cmd: u32, _arg: usize) -> SysResult<usize> {
        Err(SysError::ENOSYS)
    }

    fn flush(&self) -> SysResult<()> {
        Ok(())
    }

    fn fsync(&self) -> SysResult<()> {
        Ok(())
    }
}

impl Inode for FatDir {
    fn node_perm(&self) -> vfs::NodePermission {
        vfs::NodePermission::empty()
    }

    fn create(
        &self,
        name: &str,
        _ty: vfs::mode,
        _perm: vfs::NodePermission,
        _rdev: Option<u64>,
    ) -> SysResult<Arc<dyn Inode>> {
        self.inner
            .create_file(name)
            .map(|file| -> Arc<dyn Inode> {
                Arc::new(FatFile {
                    filename: String::from(name),
                    inner: Mutex::new(FatFileInner {
                        inner: file,
                        size: 0,
                    }),
                })
            })
            .map_err(as_sys_err)
    }

    fn link(&self, _name: &str, _src: Arc<dyn Inode>) -> SysResult<Arc<dyn Inode>> {
        Err(SysError::ENOSYS)
    }

    fn unlink(&self, _name: &str) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    fn symlink(&self, _name: &str, _sy_name: &str) -> SysResult<Arc<dyn Inode>> {
        Err(SysError::ENOSYS)
    }

    fn lookup(&self, _name: &str) -> SysResult<Arc<dyn Inode>> {
        Err(SysError::ENOSYS)
    }

    fn rmdir(&self, _name: &str) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    fn readlink(&self, _buf: &mut [u8]) -> SysResult<usize> {
        Err(SysError::ENOSYS)
    }

    fn set_attr(&self, _attr: vfs::InodeAttr) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    fn get_attr(&self) -> SysResult<vfs::FileStat> {
        Err(SysError::ENOSYS)
    }

    fn list_xattr(&self) -> SysResult<Vec<String>> {
        Err(SysError::ENOSYS)
    }

    fn truncate(&self, _len: u64) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    fn rename_to(
        &self,
        _old_name: &str,
        _new_parent: Arc<dyn Inode>,
        _new_name: &str,
        _flag: vfs::RenameFlag,
    ) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    fn update_time(&self, _time: vfs::Time, _now: vfs::TimeSpec) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    fn inode_type(&self) -> vfs::mode {
        todo!()
    }

    fn open(&self, name: &str, _flags: vfs::OpenFlags) -> SysResult<Arc<dyn Inode>> {
        let file = self.inner.iter().find(|f| {
            log::info!("{}", f.as_ref().unwrap().file_name());
            f.as_ref().unwrap().file_name() == name
        });
        let file = file.map(|x| x.unwrap()).ok_or(SysError::EIO)?;
        if file.is_dir() {
            Ok(Arc::new(FatDir {
                filename: String::from(name),
                inner: file.to_dir(),
            }))
        } else if file.is_file() {
            Ok(Arc::new(FatFile {
                filename: String::from(name),
                inner: Mutex::new(FatFileInner {
                    inner: file.to_file(),
                    size: file.len() as usize,
                }),
            }))
        } else {
            unreachable!()
        }
    }
}

pub const fn as_sys_err(err: fatfs::Error<()>) -> systype::SysError {
    match err {
        _ => SysError::EIO,
    }
}

pub struct DiskCursor {
    sector: u64,
    offset: usize,
    blk_dev: Arc<dyn BlockDevice>,
}

unsafe impl Sync for DiskCursor {}
unsafe impl Send for DiskCursor {}

impl DiskCursor {
    fn get_position(&self) -> usize {
        (self.sector * 0x200) as usize + self.offset
    }

    fn set_position(&mut self, position: usize) {
        self.sector = (position / 0x200) as u64;
        self.offset = position % 0x200;
    }

    fn move_cursor(&mut self, amount: usize) {
        self.set_position(self.get_position() + amount)
    }
}

impl fatfs::IoBase for DiskCursor {
    type Error = ();
}

impl fatfs::Read for DiskCursor {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // 由于读取扇区内容还需要考虑跨 cluster，因此 read 函数只读取一个扇区
        // 防止读取较多数据时超出限制
        // 读取所有的数据的功能交给 read_exact 来实现

        // 如果 start 不是 0 或者 len 不是 512
        let read_size = if self.offset != 0 || buf.len() < 512 {
            let mut data = vec![0u8; 512];
            self.blk_dev.read_blocks(self.sector as usize, &mut data);

            let start = self.offset;
            let end = (self.offset + buf.len()).min(512);

            buf[..end - start].copy_from_slice(&data[start..end]);
            end - start
        } else {
            // floor the buf len
            let rlen = (buf.len() / 512) * 512;
            assert!(rlen % 0x200 == 0);
            // 如果不用同一个数组 会导致读取数据的时候出现问题
            let mut data = vec![0u8; rlen];
            self.blk_dev.read_blocks(self.sector as usize, &mut data);
            buf[..rlen].copy_from_slice(&data);
            rlen
        };

        self.move_cursor(read_size);
        Ok(read_size)
    }
}

impl fatfs::Write for DiskCursor {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        // 由于写入扇区还需要考虑申请 cluster，因此 write 函数只写入一个扇区
        // 防止写入较多数据时超出限制
        // 写入所有的数据的功能交给 write_all 来实现

        // 获取硬盘设备写入器（驱动？）
        // 如果 start 不是 0 或者 len 不是 512
        let write_size = if self.offset != 0 || buf.len() < 512 {
            let mut data = vec![0u8; 512];
            self.blk_dev.read_blocks(self.sector as usize, &mut data);

            let start = self.offset;
            let end = (self.offset + buf.len()).min(512);

            data[start..end].clone_from_slice(&buf[..end - start]);
            self.blk_dev.write_blocks(self.sector as usize, &mut data);

            end - start
        } else {
            // should copy data from buffer
            let mut data = vec![0u8; 512];
            data.copy_from_slice(&buf[..512]);
            self.blk_dev.write_blocks(self.sector as usize, &data);
            512
        };

        self.move_cursor(write_size);
        Ok(write_size)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl fatfs::Seek for DiskCursor {
    fn seek(&mut self, pos: fatfs::SeekFrom) -> Result<u64, Self::Error> {
        match pos {
            fatfs::SeekFrom::Start(i) => {
                self.set_position(i as usize);
                Ok(i)
            }
            fatfs::SeekFrom::End(_) => unreachable!(),
            fatfs::SeekFrom::Current(i) => {
                let new_pos = (self.get_position() as i64) + i;
                self.set_position(new_pos as usize);
                Ok(new_pos as u64)
            }
        }
    }
}
