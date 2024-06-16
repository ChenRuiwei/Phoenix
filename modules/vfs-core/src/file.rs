use alloc::{boxed::Box, sync::Arc, vec, vec::Vec};
use core::{
    cmp,
    sync::atomic::{AtomicUsize, Ordering},
    usize,
};

use async_trait::async_trait;
use config::mm::{PAGE_MASK, PAGE_SIZE};
use memory::page::Page;
use spin::Mutex;
use systype::{SysError, SysResult, SyscallResult};

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
}

#[async_trait]
pub trait File: Send + Sync {
    fn meta(&self) -> &FileMeta;

    async fn base_read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        todo!()
    }

    async fn base_write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        todo!()
    }

    /// Read directory entries. This is called by the getdents(2) system call.
    ///
    /// For every call, this function will return an valid entry, or an error.
    /// If it read to the end of directory, it will return an empty entry.
    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        todo!()
    }

    /// Load all dentry and inodes in a directory. Will not advance dir offset.
    fn base_load_dir(&self) -> SysResult<()> {
        todo!()
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }

    fn ioctl(&self, _cmd: usize, _arg: usize) -> SyscallResult {
        Err(SysError::ENOTTY)
    }

    /// Given interested events, keep track of these events and return events
    /// that is ready.
    // TODO:
    async fn base_poll(&self, events: PollEvents) -> PollEvents {
        let mut res = PollEvents::empty();
        if events.contains(PollEvents::IN) {
            res |= PollEvents::IN;
        }
        if events.contains(PollEvents::OUT) {
            res |= PollEvents::OUT;
        }
        res
    }

    fn inode(&self) -> Arc<dyn Inode> {
        self.meta().inode.clone()
    }

    fn itype(&self) -> InodeType {
        self.meta().inode.itype()
    }

    /// Called when the VFS needs to move the file position index.
    ///
    /// Return the result offset.
    fn seek(&self, pos: SeekFrom) -> SyscallResult {
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

    /// Read at an `offset`, and will fill `buf` until `buf` is full or eof is
    /// reached. Will not advance offset.
    ///
    /// Returns count of bytes actually read or an error.
    pub async fn read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        log::info!(
            "[File::read] file {}, offset {offset}, buf len {}",
            self.dentry().path(),
            buf.len()
        );
        let mut buf = buf;
        let mut count = 0;
        let mut offset = offset;
        let inode = self.inode();
        let Some(address_space) = inode.address_space() else {
            log::debug!("[File::read] read without address_space");
            let count = self.base_read_at(offset, buf).await?;
            return Ok(count);
        };
        log::debug!("[File::read] read with address_space");
        while !buf.is_empty() && offset < self.size() {
            let offset_aligned = offset & !PAGE_MASK;
            let offset_in_page = offset - offset_aligned;
            let len = if let Some(page) = address_space.get_page(offset_aligned) {
                log::trace!("[File::read] offset {offset_aligned} cached in address space");
                let len = cmp::min(buf.len(), PAGE_SIZE - offset_in_page).min(self.size() - offset);
                buf[0..len]
                    .copy_from_slice(page.bytes_array_range(offset_in_page..offset_in_page + len));
                len
            } else {
                log::trace!("[File::read] offset {offset_aligned} not cached in address space");
                let page = Page::new();
                let len = self
                    .base_read_at(offset_aligned, page.bytes_array())
                    .await?;
                if len == 0 {
                    log::warn!("[File::read] reach file end");
                    break;
                }
                let len = cmp::min(buf.len(), len);
                buf[0..len]
                    .copy_from_slice(page.bytes_array_range(offset_in_page..offset_in_page + len));
                address_space.insert_page(offset_aligned, page);
                len
            };
            log::trace!("[File::read] read count {len}, buf len {}", buf.len());
            count += len;
            offset += len;
            buf = &mut buf[len..];
        }
        log::info!("[File::read] read count {count}");
        Ok(count)
    }

    /// Called by write(2) and related system calls.
    ///
    /// On success, the number of bytes written is returned, and the file offset
    /// is incremented by the number of bytes actually written.
    pub async fn write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        self.base_write_at(offset, buf).await
    }

    /// Read from offset in self, and will fill `buf` until `buf` is full or eof
    /// is reached. Will advance offset.
    pub async fn read(&self, buf: &mut [u8]) -> SyscallResult {
        let pos = self.pos();
        let ret = self.read_at(pos, buf).await?;
        self.set_pos(pos + ret);
        Ok(ret)
    }

    pub async fn write(&self, buf: &[u8]) -> SyscallResult {
        let pos = self.pos();
        let ret = self.write_at(pos, buf).await?;
        self.set_pos(pos + ret);
        Ok(ret)
    }

    /// Given interested events, keep track of these events and return events
    /// that is ready.
    // TODO:
    pub async fn poll(&self, events: PollEvents) -> PollEvents {
        log::info!("[File::poll] path:{}", self.dentry().path());
        self.base_poll(events).await
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
    pub async fn read_all(&self) -> SysResult<Vec<u8>> {
        log::info!("[File::read_all] file size {}", self.size());
        let mut buf = vec![0; self.size()];
        self.read_at(0, &mut buf).await?;
        Ok(buf)
    }
}
