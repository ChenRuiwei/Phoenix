use alloc::{sync::Arc, vec::Vec};
use core::{
    sync::atomic::{AtomicUsize, Ordering},
    usize,
};

use config::mm::PAGE_SIZE;
use systype::SysResult;

use crate::{Dentry, DirEntry, Inode, InodeType, SeekFrom};

pub struct FileMeta {
    /// Dentry which pointes to this file.
    pub dentry: Arc<dyn Dentry>,
    pub inode: Arc<dyn Inode>,

    /// Offset position of this file.
    /// WARN: may cause trouble if this is not locked with other things.
    pub pos: AtomicUsize,
}

impl FileMeta {
    pub fn new(dentry: Arc<dyn Dentry>, inode: Arc<dyn Inode>) -> Self {
        Self {
            dentry,
            inode,
            pos: 0.into(),
        }
    }
}

pub trait File: Send + Sync {
    fn meta(&self) -> &FileMeta;

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

    /// Read directory entries. This is called by the getdents(2) system call.
    ///
    /// For every call, this function will return an valid entry, or an error.
    /// If it read to the end of directory, it will return an empty entry.
    fn read_dir(&self) -> SysResult<Option<DirEntry>>;

    fn flush(&self) -> SysResult<usize>;

    fn inode(&self) -> Arc<dyn Inode> {
        self.meta().inode.clone()
    }

    fn itype(&self) -> InodeType {
        self.meta().inode.itype()
    }

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

    fn pos(&self) -> usize {
        self.meta().pos.load(Ordering::Relaxed)
    }
}

impl dyn File {
    pub fn dentry(&self) -> Arc<dyn Dentry> {
        self.meta().dentry.clone()
    }

    /// Read all data from this file synchronously.
    pub fn read_all_from_start(&self, buffer: &mut Vec<u8>) -> SysResult<()> {
        let old_pos = self.seek(SeekFrom::Start(0_u64))?;
        buffer.clear();
        buffer.resize(PAGE_SIZE, 0);
        let mut idx = 0;
        loop {
            let len = self.read(idx, &mut buffer.as_mut_slice()[idx..idx + PAGE_SIZE])?;
            if len == 0 {
                break;
            }
            idx += len;
            buffer.resize(idx + PAGE_SIZE, 0);
        }
        self.seek(SeekFrom::Start(old_pos as u64))?;
        Ok(())
    }
}
