#![no_std]
#![no_main]
#![feature(format_args_nl)]

use alloc::{sync::Arc, vec::Vec};
use core::cmp;

use config::board::BLOCK_SIZE;
use device_core::BlockDevice;
use fatfs::{DefaultTimeProvider, Dir, DirIter, Error, File, FileSystem, LossyOemCpConverter};
use sync::mutex::SpinNoIrqLock;
use systype::SysError;

#[macro_use]
extern crate alloc;

mod dentry;
mod file;
mod fs;
mod inode;

pub use fs::FatFsType;

type Mutex<T> = SpinNoIrqLock<T>;
type Shared<T> = Arc<Mutex<T>>;

fn new_shared<T>(val: T) -> Shared<T> {
    Arc::new(Mutex::new(val))
}

type FatDir = Dir<DiskCursor, DefaultTimeProvider, LossyOemCpConverter>;
type FatFile = File<DiskCursor, DefaultTimeProvider, LossyOemCpConverter>;
type FatDirIter = DirIter<DiskCursor, DefaultTimeProvider, LossyOemCpConverter>;
type FatFs = FileSystem<DiskCursor, DefaultTimeProvider, LossyOemCpConverter>;

pub const fn as_sys_err(err: fatfs::Error<()>) -> systype::SysError {
    match err {
        Error::NotFound => SysError::ENOENT,
        _ => SysError::EIO,
    }
}

#[derive(Clone)]
pub struct DiskCursor {
    /// Block index on block device.
    block_id: u64,
    /// Offset in a block.
    offset: usize,
    /// BLock device.
    blk_dev: Arc<dyn BlockDevice>,
}

impl DiskCursor {
    fn pos(&self) -> usize {
        (self.block_id as usize * BLOCK_SIZE) + self.offset
    }

    fn set_pos(&mut self, position: usize) {
        self.block_id = (position / BLOCK_SIZE) as u64;
        self.offset = position % BLOCK_SIZE;
    }

    fn move_cur(&mut self, amount: usize) {
        self.set_pos(self.pos() + amount)
    }
}

impl fatfs::IoBase for DiskCursor {
    type Error = ();
}

impl fatfs::Read for DiskCursor {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut data = vec![0; BLOCK_SIZE];
        let len: usize = cmp::min(buf.len(), BLOCK_SIZE - self.offset);
        self.blk_dev.read_block(self.block_id as usize, &mut data);
        buf[..len].copy_from_slice(&data[self.offset..self.offset + len]);
        self.move_cur(len);
        Ok(len)
    }
}

impl fatfs::Write for DiskCursor {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let mut data = vec![0; BLOCK_SIZE];
        let len: usize = cmp::min(buf.len(), BLOCK_SIZE - self.offset);
        if len < BLOCK_SIZE {
            self.blk_dev.read_block(self.block_id as usize, &mut data);
        }
        data[self.offset..self.offset + len].copy_from_slice(&buf[..len]);
        self.blk_dev.write_block(self.block_id as usize, &mut data);
        self.move_cur(len);
        Ok(len)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl fatfs::Seek for DiskCursor {
    fn seek(&mut self, pos: fatfs::SeekFrom) -> Result<u64, Self::Error> {
        match pos {
            fatfs::SeekFrom::Start(i) => {
                self.set_pos(i as usize);
                Ok(i)
            }
            fatfs::SeekFrom::End(_) => unreachable!(),
            fatfs::SeekFrom::Current(i) => {
                self.move_cur(i as usize);
                Ok(self.pos() as u64)
            }
        }
    }
}
