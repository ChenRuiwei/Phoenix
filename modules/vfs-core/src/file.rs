use alloc::{boxed::Box, string::ToString, sync::Arc, vec::Vec};
use core::{
    sync::atomic::{AtomicUsize, Ordering},
    usize,
};

use async_trait::async_trait;
use config::mm::PAGE_SIZE;
use spin::Mutex;
use systype::{ASyscallResult, SysError, SysResult, SyscallResult};

use crate::{
    Dentry, DirEntry, Inode, InodeState, InodeType, OpenFlags, PollEvents, SeekFrom, SuperBlock,
};

pub struct FileMeta {
    /// Dentry which pointes to this file.
    pub dentry: Arc<dyn Dentry>,
    pub inode: Arc<dyn Inode>,

    /// Offset position of this file.
    /// WARN: may cause trouble if this is not locked with other things.
    pub pos: AtomicUsize,
    pub flags: Mutex<OpenFlags>,
}

impl FileMeta {
    pub fn new(dentry: Arc<dyn Dentry>, inode: Arc<dyn Inode>) -> Self {
        Self {
            dentry,
            inode,
            pos: 0.into(),
            flags: Mutex::new(OpenFlags::empty()),
        }
    }

    pub fn new_with_flags(
        dentry: Arc<dyn Dentry>,
        inode: Arc<dyn Inode>,
        flags: OpenFlags,
    ) -> Self {
        Self {
            dentry,
            inode,
            pos: 0.into(),
            flags: Mutex::new(flags),
        }
    }
}

#[async_trait]
pub trait File: Send + Sync {
    fn meta(&self) -> &FileMeta;

    /// Called by read(2) and related system calls.
    ///
    /// On success, the number of bytes read is returned (zero indicates end of
    /// file), and the file position is advanced by this number.
    async fn read(&self, offset: usize, buf: &mut [u8]) -> SyscallResult;

    /// Called by write(2) and related system calls.
    ///
    /// On success, the number of bytes written is returned, and the file offset
    /// is incremented by the number of bytes actually written.
    async fn write(&self, offset: usize, buf: &[u8]) -> SyscallResult;

    /// Read directory entries. This is called by the getdents(2) system call.
    ///
    /// For every call, this function will return an valid entry, or an error.
    /// If it read to the end of directory, it will return an empty entry.
    fn base_read_dir(&self) -> SysResult<Option<DirEntry>>;

    /// Load all dentry and inodes in a directory. Will not advance dir offset.
    fn base_load_dir(&self) -> SysResult<()> {
        todo!()
    }

    fn flush(&self) -> SysResult<usize>;

    fn ioctl(&self, cmd: usize, arg: usize) -> SyscallResult {
        Err(SysError::ENOTTY)
    }

    async fn poll(&self, events: PollEvents) -> SysResult<PollEvents> {
        let mut res = PollEvents::empty();
        if events.contains(PollEvents::POLLIN) {
            res |= PollEvents::POLLIN;
        }
        if events.contains(PollEvents::POLLOUT) {
            res |= PollEvents::POLLOUT;
        }
        Ok(res)
    }

    fn inode(&self) -> Arc<dyn Inode> {
        self.meta().inode.clone()
    }

    // NOTE: super block has an arc of inode
    fn i_cnt(&self) -> usize {
        Arc::strong_count(&self.meta().inode)
    }

    fn itype(&self) -> InodeType {
        self.meta().inode.itype()
    }

    /// Called when the VFS needs to move the file position index.
    ///
    /// Return the result offset.
    fn seek(&self, pos: SeekFrom) -> SysResult<usize> {
        let mut res_pos = self.pos();
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
                let size = self.size();
                if off < 0 {
                    res_pos = size - off.abs() as usize;
                } else {
                    res_pos = size + off as usize;
                }
            }
        }
        self.set_pos(res_pos);
        Ok(res_pos)
    }

    fn pos(&self) -> usize {
        self.meta().pos.load(Ordering::Relaxed)
    }

    fn set_pos(&self, pos: usize) {
        self.meta().pos.store(pos, Ordering::Relaxed)
    }

    fn dentry(&self) -> Arc<dyn Dentry> {
        self.meta().dentry.clone()
    }

    fn super_block(&self) -> Arc<dyn SuperBlock> {
        self.meta().dentry.super_block()
    }

    fn size(&self) -> usize {
        self.meta().inode.size()
    }
}

impl dyn File {
    pub fn flags(&self) -> OpenFlags {
        self.meta().flags.lock().clone()
    }

    pub fn set_flags(&self, flags: OpenFlags) {
        *self.meta().flags.lock() = flags;
    }

    pub fn load_dir(&self) -> SysResult<()> {
        let inode = self.inode();
        if inode.state() == InodeState::Init {
            self.base_load_dir()?;
            inode.set_state(InodeState::Synced)
        }
        Ok(())
    }

    pub fn read_dir(&self) -> SysResult<Option<DirEntry>> {
        self.load_dir()?;
        // PERF: should cache the iter stream
        if let Some(sub_dentry) = self
            .dentry()
            .children()
            .values()
            .filter(|c| !c.is_negetive())
            .nth(self.pos())
        {
            self.seek(SeekFrom::Current(1))?;
            let inode = sub_dentry.inode()?;
            let dirent = DirEntry {
                ino: inode.ino() as u64,
                off: self.pos() as u64,
                itype: inode.itype(),
                name: sub_dentry.name_string(),
            };
            Ok(Some(dirent))
        } else {
            Ok(None)
        }
    }

    /// Read all data from this file synchronously.
    pub async fn read_all_from_start(&self, buffer: &mut Vec<u8>) -> SysResult<()> {
        let old_pos = self.seek(SeekFrom::Start(0_u64))?;
        buffer.clear();
        buffer.resize(PAGE_SIZE, 0);
        let mut idx = 0;
        loop {
            let len = self
                .read(idx, &mut buffer.as_mut_slice()[idx..idx + PAGE_SIZE])
                .await?;
            // log::trace!("[read_all_from_start] read len: {}", len);
            if len < PAGE_SIZE {
                break;
            }
            debug_assert_eq!(len, PAGE_SIZE);
            idx += len;
            buffer.resize(idx + PAGE_SIZE, 0);
            // log::trace!("[read_all_from_start] buf len: {}", buffer.len());
        }
        self.seek(SeekFrom::Start(old_pos as u64))?;
        Ok(())
    }
}
