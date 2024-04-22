use alloc::{
    string::String,
    sync::{Arc, Weak},
};
use core::{
    default, result,
    sync::atomic::{AtomicUsize, Ordering},
    usize,
};

use systype::{SysError, SysResult};

use crate::{Inode, SeekFrom};

pub struct FileMeta {
    /// Path of this file.
    pub path: String,
    pub inode: Arc<dyn Inode>,

    /// Offset position of this file.
    /// WARN: may cause trouble if this is not locked with other things.
    pub pos: AtomicUsize,
}

pub trait File: Send + Sync {
    fn meta(&self) -> &FileMeta;

    fn set_meta(&self, meta: FileMeta);

    /// Called when the VFS needs to move the file position index.
    ///
    /// Return the result offset.
    fn lseek(&self, pos: SeekFrom) -> SysResult<usize> {
        let meta = self.meta();
        let mut res_pos = meta.pos.load(Ordering::Relaxed);
        match pos {
            SeekFrom::Current(off) => {
                if off < 0 {
                    res_pos -= off.abs() as usize;
                } else {
                    res_pos += off as usize;
                }
            }
            SeekFrom::Start(off) => {
                res_pos = off as usize;
            }
            SeekFrom::End(off) => {
                todo!();
            }
        }
        meta.pos.store(res_pos, Ordering::Relaxed);
        Ok(res_pos)
    }

    /// Called by read(2) and related system calls.
    ///
    /// On success, the number of bytes read is returned (zero indicates end of
    /// file), and the file position is advanced by this number.
    fn read(&self, _offset: usize, _buf: &mut [u8]) -> SysResult<usize>;

    /// Called by write(2) and related system calls.
    ///
    /// On success, the number of bytes written is returned, and the file offset
    /// is incremented by the number of bytes actually written.
    fn write(&self, _offset: usize, _buf: &[u8]) -> SysResult<usize>;

    fn flush(&self) -> SysResult<usize>;
}

impl dyn File {
    pub fn inode(&self) -> Arc<dyn Inode> {
        self.meta().inode.clone()
    }
}
