use alloc::{boxed::Box, sync::Arc, vec, vec::Vec};
use core::{
    cmp,
    sync::atomic::{AtomicUsize, Ordering},
    usize,
};

use async_trait::async_trait;
use config::{
    board::BLOCK_SIZE,
    mm::{align_offset_to_page, PAGE_MASK, PAGE_SIZE},
};
use driver::qemu::virtio_blk::VirtIOBlkDev;
use page::Page;
use spin::Mutex;
use systype::{SysError, SysResult, SyscallResult};

use crate::{
    address_space, inode, AddressSpace, Dentry, DirEntry, Inode, InodeState, InodeType, OpenFlags,
    PollEvents, SeekFrom, SuperBlock,
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
    ///
    /// lseek() allows the file offset to be set beyond the end of the file (but
    /// this does not change the size of the file). If data is later written at
    /// this point, subsequent reads of the data in the gap (a "hole") return
    /// null bytes ('\0') until data is actually written into the gap.
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

    fn flags(&self) -> OpenFlags {
        self.meta().flags.lock().clone()
    }

    fn set_flags(&self, flags: OpenFlags) {
        *self.meta().flags.lock() = flags;
    }
}

impl dyn File {
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

        let mut buf_it = buf;
        let mut offset_it = offset;
        let inode = self.inode();

        let Some(address_space) = inode.address_space() else {
            log::debug!("[File::read] read without address_space");
            let count = self.base_read_at(offset_it, buf_it).await?;
            return Ok(count);
        };

        log::debug!("[File::read] read with address_space");
        while !buf_it.is_empty() && offset_it < self.size() {
            let (offset_aligned, offset_in_page) = align_offset_to_page(offset_it);
            let len =
                cmp::min(buf_it.len(), PAGE_SIZE - offset_in_page).min(self.size() - offset_it);
            let page = if let Some(page) = address_space.get_page(offset_aligned) {
                page
            } else if let Some(page) = self.read_page_at(offset_aligned).await? {
                page
            } else {
                // no page means EOF
                break;
            };
            buf_it[0..len]
                .copy_from_slice(page.bytes_array_range(offset_in_page..offset_in_page + len));
            log::trace!("[File::read] read count {len}, buf len {}", buf_it.len());
            offset_it += len;
            buf_it = &mut buf_it[len..];
        }
        log::info!("[File::read] read count {}", offset_it - offset);
        Ok(offset_it - offset)
    }

    /// Read a page at `offset_aligned` without address space.
    async fn read_page_at(&self, offset_aligned: usize) -> SysResult<Option<Arc<Page>>> {
        log::trace!("[File::read_page] read offset {offset_aligned}");

        if offset_aligned >= self.size() {
            log::warn!("[File::read_page] reach end of file");
            return Ok(None);
        }

        let inode = self.inode();
        let address_space = inode.address_space().unwrap();

        let device = inode.super_block().device();
        let mut page = Page::new_arc();
        page.init_block_device(&device);
        // read a page normally or less than a page when EOF reached
        let len = self
            .base_read_at(offset_aligned, page.bytes_array())
            .await?;

        let virtio_blk = device
            .downcast_arc::<VirtIOBlkDev>()
            .unwrap_or_else(|_| unreachable!());
        let buffer_caches = virtio_blk.cache.lock();
        for offset in (offset_aligned..offset_aligned + len).step_by(BLOCK_SIZE) {
            let block_id = inode.get_blk_idx(offset)?;
            let buffer_head = buffer_caches.get_buffer_head(block_id as usize);
            page.insert_buffer_head(buffer_head);
        }

        address_space.insert_page(offset_aligned, page.clone());

        Ok(Some(page))
    }

    /// Called by write(2) and related system calls.
    ///
    /// On success, the number of bytes written is returned, and the file offset
    /// is incremented by the number of bytes actually written.
    pub async fn write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        log::info!(
            "[File::write] file {}, offset {offset}, buf len {}",
            self.dentry().path(),
            buf.len()
        );
        let mut buf = buf;
        let mut count = 0;
        let old_offset = offset;
        let mut offset = offset;
        let inode = self.inode();

        let Some(address_space) = inode.address_space() else {
            log::debug!("[File::write] write without address_space");
            let count = self.base_write_at(offset, buf).await?;
            return Ok(count);
        };

        log::debug!("[File::write] write with address_space");
        while !buf.is_empty() {
            let offset_aligned = offset & !PAGE_MASK;
            let offset_in_page = offset - offset_aligned;

            let len = if let Some(page) = address_space.get_page(offset_aligned) {
                log::trace!("[File::write] offset {offset_aligned} cached in address space");
                let len = cmp::min(buf.len(), PAGE_SIZE - offset_in_page);
                page.bytes_array_range(offset_in_page..offset_in_page + len)
                    .copy_from_slice(&buf[0..len]);
                len
            } else {
                log::trace!("[File::write] offset {offset_aligned} not cached in address space");
                let page = Page::new_arc();
                if offset < self.size() {
                    self.base_read_at(offset_aligned, page.bytes_array())
                        .await?;
                }
                let len = cmp::min(buf.len(), PAGE_SIZE - offset_in_page);
                page.bytes_array_range(offset_in_page..offset_in_page + len)
                    .copy_from_slice(&buf[0..len]);
                address_space.insert_page(offset_aligned, page);
                len
            };
            log::trace!("[File::write] write count {len}, buf len {}", buf.len());
            count += len;
            offset += len;
            buf = &buf[len..];
        }
        if old_offset + count > self.size() {
            log::info!("[File::write] write beyond file size");
            self.inode().set_size(offset + count);
        }
        log::info!("[File::write] write count {count}");
        Ok(count)
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
        if self.flags().contains(OpenFlags::O_APPEND) {
            self.set_pos(self.size());
        }
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
