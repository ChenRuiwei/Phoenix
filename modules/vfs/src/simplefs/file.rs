use alloc::{boxed::Box, sync::Arc, vec::Vec};

use async_trait::async_trait;
use config::mm::{align_offset_to_page, PAGE_SIZE};
use page::Page;
use sync::mutex::SpinNoIrqLock;
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::{Dentry, DirEntry, File, FileMeta, Inode};

pub struct SimpleDirFile {
    meta: FileMeta,
}

impl SimpleDirFile {
    pub fn new(dentry: Arc<dyn Dentry>, inode: Arc<dyn Inode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry, inode),
        })
    }
}

#[async_trait]
impl File for SimpleDirFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, _offset: usize, _buf: &mut [u8]) -> SyscallResult {
        Err(SysError::EISDIR)
    }

    async fn base_write_at(&self, _offset: usize, _buf: &[u8]) -> SyscallResult {
        Err(SysError::EISDIR)
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        todo!()
    }

    fn base_load_dir(&self) -> SysResult<()> {
        Ok(())
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }
}

pub struct SimpleFileFile {
    meta: FileMeta,
}

impl SimpleFileFile {
    pub fn new(dentry: Arc<dyn Dentry>, inode: Arc<dyn Inode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry, inode),
        })
    }
}

#[async_trait]
impl File for SimpleFileFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        unreachable!()
    }

    async fn base_write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        unreachable!()
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_load_dir(&self) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }

    async fn read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        log::info!(
            "[File::read] file {}, offset {offset}, buf len {}",
            self.dentry().path(),
            buf.len()
        );

        let inode = self.inode();

        let page_cache = inode.page_cache().unwrap();

        let mut buf_it = buf;
        let mut offset_it = offset;

        log::debug!("[File::read] read with address_space");
        while !buf_it.is_empty() && offset_it < self.size() {
            let (offset_aligned, offset_in_page) = align_offset_to_page(offset_it);
            let page = if let Some(page) = page_cache.get_page(offset_aligned) {
                page
            } else {
                // no page means EOF
                break;
            };
            let len = (buf_it.len())
                .min(PAGE_SIZE - offset_in_page)
                .min(self.size() - offset_it);
            buf_it[0..len]
                .copy_from_slice(page.bytes_array_range(offset_in_page..offset_in_page + len));
            log::trace!("[File::read] read count {len}, buf len {}", buf_it.len());
            offset_it += len;
            buf_it = &mut buf_it[len..];
        }
        log::info!("[File::read] read count {}", offset_it - offset);
        Ok(offset_it - offset)
    }

    async fn write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        log::info!(
            "[File::write] file {}, offset {offset}, buf len {}",
            self.dentry().path(),
            buf.len()
        );

        let inode = self.inode();

        let page_cache = inode.page_cache().unwrap();
        if offset > self.size() {
            todo!("offset greater than size, will create hole");
        }

        let mut buf_it = buf;
        let mut offset_it = offset;

        while !buf_it.is_empty() {
            let (offset_aligned, offset_in_page) = align_offset_to_page(offset_it);
            let page = if let Some(page) = page_cache.get_page(offset_aligned) {
                page
            } else {
                log::info!("[File::write_at] create new page");
                let page = Page::new();
                page_cache.insert_page(offset_aligned, page.clone());
                page
            };
            let len = (buf_it.len()).min(PAGE_SIZE - offset_in_page);
            page.bytes_array_range(offset_in_page..offset_in_page + len)
                .copy_from_slice(&buf_it[0..len]);
            log::trace!("[File::write] write count {len}, buf len {}", buf_it.len());
            offset_it += len;
            buf_it = &buf_it[len..];
        }
        if offset_it > self.size() {
            log::warn!(
                "[File::write_at] write beyond file, offset_it:{offset_it}, size:{}",
                self.size()
            );
            let new_size = offset_it;
            inode.set_size(new_size);
        }
        Ok(buf.len())
    }

    async fn get_page_at(&self, offset_aligned: usize) -> SysResult<Option<Arc<Page>>> {
        let inode = self.inode();
        let page_cache = inode.page_cache().unwrap();
        if let Some(page) = page_cache.get_page(offset_aligned) {
            Ok(Some(page))
        } else {
            // no page means EOF
            Ok(None)
        }
    }
}
