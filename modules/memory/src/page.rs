use core::cmp;

use config::mm::PAGE_SIZE;

use crate::{alloc_frame, FrameTracker, PhysPageNum};

pub struct Page {
    frame: FrameTracker,
}

impl core::fmt::Debug for Page {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Page").field("frame", &self.ppn()).finish()
    }
}

impl Page {
    /// Create a `Page` by allocating a frame.
    pub fn new() -> Self {
        Self {
            frame: alloc_frame(),
        }
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
}