use alloc::sync::{Arc, Weak};
use core::{cmp, ops::Range};

use config::{
    board::BLOCK_SIZE,
    mm::{block_page_offset, MAX_BUFFERS_PER_PAGE, PAGE_SIZE},
};
use device_core::BlockDevice;
use enum_as_inner::EnumAsInner;
use intrusive_collections::LinkedList;
use memory::{alloc_frame_tracker, FrameTracker, PhysPageNum};
use sync::mutex::SpinNoIrqLock;

use crate::{
    buffer_cache::{BufferHead, BufferHeadAdapter},
    BufferState,
};

#[derive(EnumAsInner)]
pub enum PageKind {
    Normal,
    FileCache(SpinNoIrqLock<BufferInfo>),
    BlockCache(SpinNoIrqLock<BufferInfo>),
}

pub struct Page {
    frame: FrameTracker,
    kind: PageKind,
}

pub struct BufferInfo {
    device: Weak<dyn BlockDevice>,
    buffer_heads: LinkedList<BufferHeadAdapter>,
    buffer_head_cnts: usize,
}

impl core::fmt::Debug for Page {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Page").field("frame", &self.ppn()).finish()
    }
}

// NOTE: Avoid flushing the page cache to disk when being dropped as the page
// might be discarded during file removal.
impl Drop for Page {
    fn drop(&mut self) {
        match &self.kind {
            PageKind::Normal => {}
            PageKind::FileCache(inner) => {
                let mut inner = inner.lock();
                while let Some(buffer_head) = inner.buffer_heads.pop_front() {
                    buffer_head.reset();
                }
            }
            PageKind::BlockCache(inner) => {
                let mut inner = inner.lock();
                log::warn!("[Page::drop] sync block buffer back to disk");
                let device = inner.device.upgrade().unwrap();
                while let Some(buffer_head) = inner.buffer_heads.pop_front() {
                    if buffer_head.bstate() == BufferState::Dirty {
                        let block_id = buffer_head.block_id();
                        device.base_write_block(buffer_head.block_id(), &self.block_bytes_array(block_id));
                    }
                    buffer_head.reset();
                }
            }
        }
    }
}

impl Page {
    /// Create a `Page` by allocating a frame.
    pub fn new() -> Arc<Self> {
        let frame = alloc_frame_tracker();
        Arc::new(Self {
            frame,
            kind: PageKind::Normal,
        })
    }

    pub fn new_file(block_device: &Arc<dyn BlockDevice>) -> Arc<Self> {
        let frame = alloc_frame_tracker();
        Arc::new(Self {
            frame,
            kind: PageKind::FileCache(SpinNoIrqLock::new(BufferInfo {
                device: Arc::downgrade(block_device),
                buffer_heads: LinkedList::new(BufferHeadAdapter::new()),
                buffer_head_cnts: 0,
            })),
        })
    }

    pub fn new_block(block_device: &Arc<dyn BlockDevice>) -> Arc<Self> {
        let frame = alloc_frame_tracker();
        Arc::new(Self {
            frame,
            kind: PageKind::BlockCache(SpinNoIrqLock::new(BufferInfo {
                device: Arc::downgrade(block_device),
                buffer_heads: LinkedList::new(BufferHeadAdapter::new()),
                buffer_head_cnts: 0,
            })),
        })
    }

    pub fn ppn(&self) -> PhysPageNum {
        self.frame.ppn
    }

    pub fn kind(&self) -> &PageKind {
        &self.kind
    }

    // WARN: user program may rely on cleared page, page is not cleared may cause
    // unknown bug
    pub fn fill_zero(&self) {
        self.frame.fill_zero()
    }

    pub fn copy_from_slice(&self, data: &[u8]) {
        let len = cmp::min(PAGE_SIZE, data.len());
        self.bytes_array_range(0..len).copy_from_slice(data)
    }

    pub fn bytes_array(&self) -> &'static mut [u8] {
        self.ppn().bytes_array()
    }

    pub fn bytes_array_range(&self, range: Range<usize>) -> &'static mut [u8] {
        self.ppn().bytes_array_range(range)
    }

    pub fn block_bytes_array(&self, block_id: usize) -> &'static mut [u8] {
        debug_assert!(self.kind.is_file_cache() || self.kind.is_block_cache());
        let offset_block_aligned = block_page_offset(block_id);
        self.bytes_array_range(offset_block_aligned..offset_block_aligned + BLOCK_SIZE)
    }

    pub fn insert_buffer_head(self: &Arc<Page>, buffer_head: Arc<BufferHead>) {
        let mut inner = match &self.kind {
            PageKind::Normal => unreachable!(),
            PageKind::FileCache(inner) => inner.lock(),
            PageKind::BlockCache(inner) => inner.lock(),
        };
        if buffer_head.has_cached() && Arc::ptr_eq(self, &buffer_head.page()) {
            log::error!("duplicate insert, block id:{}", buffer_head.block_id());
            return;
        }
        let count = inner.buffer_heads.iter().count();
        buffer_head.init(self, count * BLOCK_SIZE);
        inner.buffer_heads.push_back(buffer_head);
        inner.buffer_head_cnts += 1;
    }

    pub fn buffer_head_cnts(&self) -> usize {
        let mut inner = match &self.kind {
            PageKind::Normal => unreachable!(),
            PageKind::FileCache(inner) => inner.lock(),
            PageKind::BlockCache(inner) => inner.lock(),
        };
        inner.buffer_head_cnts
    }

    pub fn flush(&self) {
        let mut inner = match &self.kind {
            PageKind::Normal => unreachable!(),
            PageKind::FileCache(inner) => inner.lock(),
            PageKind::BlockCache(inner) => inner.lock(),
        };
        log::warn!("[Page::flush] sync buffer back to disk");
        let device = inner.device.upgrade().unwrap();
        for buffer_head in inner.buffer_heads.iter() {
            if buffer_head.bstate() == BufferState::Dirty {
                let block_id = buffer_head.block_id();
                device.base_write_block(buffer_head.block_id(), &self.block_bytes_array(block_id));
            }
        }
    }
}
