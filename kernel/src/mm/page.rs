use alloc::sync::{Arc, Weak};

use config::{board::BLOCK_SIZE, mm::PAGE_SIZE};
use memory::{frame_alloc, FrameTracker, PhysPageNum};
use sync::mutex::{sleep_mutex::SleepMutex, SleepLock};
use systype::{SysError, SysResult};
use vfs::inode::Inode;

pub struct Page {
    frame: FrameTracker,
    file_info: Option<SleepLock<FilePageInfo>>,
}

impl core::fmt::Debug for Page {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Page").field("frame", &self.ppn()).finish()
    }
}

pub struct FilePageInfo {
    /// Offset of the file at page start.
    file_offset: usize,
    /// Data block state
    data_states: [DataState; PAGE_SIZE / BLOCK_SIZE],
    /// Inode that this page related to
    inode: Weak<dyn Inode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataState {
    /// Uninitialized, which means not loaded from disk yet.
    UnInit,
    /// Data in memory is coherent with date on disk.
    Coherent,
    /// Data in memory is dirty.
    Dirty,
}

impl FilePageInfo {
    pub fn inode(&self) -> Arc<dyn Inode> {
        self.inode.upgrade().unwrap()
    }
}

impl Page {
    pub fn new() -> Self {
        Self {
            frame: frame_alloc(),
            file_info: None,
        }
    }

    pub fn new_with_file(inode: Weak<dyn Inode>, file_offset: usize) -> Self {
        let file_page_info = FilePageInfo {
            file_offset,
            data_states: core::array::from_fn(|_| DataState::UnInit),
            inode,
        };
        Self {
            frame: frame_alloc(),
            file_info: Some(SleepLock::new(file_page_info)),
        }
    }

    pub fn ppn(&self) -> PhysPageNum {
        self.frame.ppn
    }

    pub fn bytes_array_range(&self, range: core::ops::Range<usize>) -> &'static mut [u8] {
        self.ppn().bytes_array_range(range)
    }

    /// Read this page starts with offset.
    pub async fn read(&self, offset: usize, buf: &mut [u8]) -> SysResult<usize> {
        debug_assert!(offset < PAGE_SIZE);
        let end = (offset + buf.len()).max(PAGE_SIZE);
        self.load_buffer(offset..end).await?;
        buf.copy_from_slice(&self.bytes_array_range(offset..end));
        Ok(end - offset)
    }

    /// Write this page starts with offset.
    pub async fn write(&self, offset: usize, buf: &[u8]) -> SysResult<usize> {
        debug_assert!(offset < PAGE_SIZE);
        let end = (offset + buf.len()).max(PAGE_SIZE);
        self.mark_buffer_dirty(offset..end).await?;
        self.bytes_array_range(offset..end).copy_from_slice(buf);
        Ok(end - offset)
    }

    /// Sync all buffers in need.
    pub async fn sync(&self) -> SysResult<()> {
        let file_info = self.file_info.as_ref().unwrap().lock().await;
        log::trace!(
            "[Page::sync] sync page, file offset {:#x}",
            file_info.file_offset
        );
        for idx in 0..PAGE_SIZE / BLOCK_SIZE {
            match file_info.data_states[idx] {
                DataState::Dirty => {
                    let page_offset = idx * BLOCK_SIZE;
                    let file_offset = file_info.file_offset + page_offset;
                    log::trace!(
                        "[Page::sync] sync block of the page, file offset {file_offset:#x}",
                    );
                    // TODO: In case of truncate (Titanix)?
                    file_info.inode().write_at(
                        file_offset as u64,
                        self.bytes_array_range(page_offset..page_offset + BLOCK_SIZE),
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Load buffers if needed.
    async fn load_buffer(&self, offset_range: core::ops::Range<usize>) -> SysResult<()> {
        let start_idx = offset_range.start / BLOCK_SIZE;
        let end_idx = (offset_range.end + BLOCK_SIZE - 1) / BLOCK_SIZE;

        let mut file_info = self.file_info.as_ref().unwrap().lock().await;
        for idx in start_idx..end_idx {
            if file_info.data_states[idx] == DataState::UnInit {
                let page_offset = idx * BLOCK_SIZE;
                let file_offset = page_offset + file_info.file_offset;
                log::trace!("outdated block, idx {idx}, file_off {file_offset:#x}",);
                file_info.inode().read_at(
                    file_offset as u64,
                    self.bytes_array_range(page_offset..page_offset + BLOCK_SIZE),
                )?;
                file_info.data_states[idx] = DataState::Coherent;
            }
        }
        Ok(())
    }

    // NOTE: no need to load buffers since these buffers will be writen immediately
    async fn mark_buffer_dirty(&self, offset_range: core::ops::Range<usize>) -> SysResult<()> {
        let start_idx = offset_range.start / BLOCK_SIZE;
        let end_idx = (offset_range.end + BLOCK_SIZE - 1) / BLOCK_SIZE;

        let mut file_info = self.file_info.as_ref().unwrap().lock().await;
        for idx in start_idx..end_idx {
            file_info.data_states[idx] = DataState::Dirty;
        }
        Ok(())
    }
}
