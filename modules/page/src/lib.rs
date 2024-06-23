#![no_std]
#![no_main]

mod buffer_cache;

pub use buffer_cache::*;

extern crate alloc;

use alloc::sync::{Arc, Weak};
use core::cmp;

use config::{
    board::BLOCK_SIZE,
    mm::{MAX_BUFFERS_PER_PAGE, PAGE_SIZE},
};
use device_core::BlockDevice;
use intrusive_collections::LinkedList;
use memory::{alloc_frame, FrameTracker, PhysPageNum};
use sync::mutex::SpinNoIrqLock;

use crate::buffer_cache::{BufferHead, BufferHeadAdapter};

pub struct Page {
    frame: FrameTracker,
    inner: SpinNoIrqLock<PageInner>,
}

pub struct PageInner {
    device: Option<Weak<dyn BlockDevice>>,
    buffer_heads: LinkedList<BufferHeadAdapter>,
}

impl core::fmt::Debug for Page {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Page").field("frame", &self.ppn()).finish()
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        let inner = self.inner.lock();
        if let Some(device) = &inner.device {
            let device = device.upgrade().unwrap();
            for buffer_head in inner.buffer_heads.iter() {
                if buffer_head.bstate() == BufferState::Dirty {
                    device.base_write_block(buffer_head.block_id(), buffer_head.bytes_array())
                }
            }
        }
    }
}

impl Page {
    /// Create a `Page` by allocating a frame.
    pub fn new() -> Self {
        let frame = alloc_frame();
        // frame.clear_page();
        Self {
            frame,
            inner: SpinNoIrqLock::new(PageInner {
                device: None,
                buffer_heads: LinkedList::new(BufferHeadAdapter::new()),
            }),
        }
    }

    pub fn new_arc() -> Arc<Self> {
        Arc::new(Self::new())
    }

    // WARN: user program may rely on cleared page, page is not cleared may cause
    // unknown bug
    pub fn fill_zero(&self) {
        self.frame.fill_zero()
    }

    pub fn copy_data_from_another(&self, another: &Page) {
        self.ppn().copy_page_from_another(another.ppn());
    }

    pub fn copy_from_slice(&self, data: &[u8]) {
        let len = cmp::min(PAGE_SIZE, data.len());
        self.bytes_array_range(0..len).copy_from_slice(data)
    }

    pub fn ppn(&self) -> PhysPageNum {
        self.frame.ppn
    }

    pub fn bytes_array(&self) -> &'static mut [u8] {
        self.ppn().bytes_array()
    }

    pub fn bytes_array_range(&self, range: core::ops::Range<usize>) -> &'static mut [u8] {
        self.ppn().bytes_array_range(range)
    }

    pub fn block_range(&self, block_id: usize) -> &'static mut [u8] {
        let offset = block_page_offset(block_id);
        self.bytes_array_range(offset..offset + BLOCK_SIZE)
    }

    pub fn init_block_device(&self, block_device: &Arc<dyn BlockDevice>) {
        let mut inner = self.inner.lock();
        inner.device = Some(Arc::downgrade(block_device))
    }

    pub fn insert_buffer_head(self: &mut Arc<Page>, buffer_head: Arc<BufferHead>) {
        let mut inner = self.inner.lock();
        let count = inner.buffer_heads.iter().count();
        buffer_head.init(self, count * BLOCK_SIZE);
        inner.buffer_heads.push_back(buffer_head);
    }
}

pub fn block_page_id(block_id: usize) -> usize {
    block_id / MAX_BUFFERS_PER_PAGE
}

pub fn block_page_offset(block_id: usize) -> usize {
    block_id % MAX_BUFFERS_PER_PAGE * BLOCK_SIZE
}
