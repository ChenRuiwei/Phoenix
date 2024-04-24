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

impl FileMeta {
    pub fn new(path: String, inode: Arc<dyn Inode>) -> Self {
        Self {
            path,
            inode,
            pos: 0.into(),
        }
    }
}

pub trait File: Send + Sync {
    fn meta(&self) -> &FileMeta;

    /// Called when the VFS needs to move the file position index.
    ///
    /// Return the result offset.
    fn seek(&self, pos: SeekFrom) -> SysResult<usize> {
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
                let size = meta.inode.size();
                if off < 0 {
                    res_pos = size - off.abs() as usize;
                } else {
                    res_pos = size + off as usize;
                }
            }
        }
        meta.pos.store(res_pos, Ordering::Relaxed);
        Ok(res_pos)
    }

    /// Called by read(2) and related system calls.
    ///
    /// On success, the number of bytes read is returned (zero indicates end of
    /// file), and the file position is advanced by this number.
    fn read(&self, offset: usize, buf: &mut [u8]) -> SysResult<usize>;

    /// Called by write(2) and related system calls.
    ///
    /// On success, the number of bytes written is returned, and the file offset
    /// is incremented by the number of bytes actually written.
    fn write(&self, offset: usize, buf: &[u8]) -> SysResult<usize>;

    /// Called by the VFS when an inode should be opened.
    fn open(&self, inode: Arc<dyn Inode>) -> SysResult<Arc<dyn File>>;

    fn flush(&self) -> SysResult<usize>;

    fn inode(&self) -> Arc<dyn Inode> {
        self.meta().inode.clone()
    }
}

impl dyn File {}
