use alloc::{
    collections::BTreeMap,
    rc::Rc,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    num::NonZeroUsize,
    sync::atomic::{AtomicUsize, Ordering},
};

use config::{
    board::BLOCK_SIZE,
    mm::{BUFFERS_IN_PAGE, BUFFER_NEED_CACHE_CNT, BUFFER_PAGES_MAX},
};
use lru::LruCache;
use memory::page::Page;
use spin::Once;
use sync::mutex::SpinNoIrqLock;

use crate::{BlockDevice, BLOCK_DEVICE};

pub struct BufferCache {
    device: Option<Weak<dyn BlockDevice>>,
    /// Block page id to `BufferPage`.
    pages: LruCache<usize, BufferPage>,
    /// Block id to `BufferHead`.
    buffer_heads: BTreeMap<usize, Arc<BufferHead>>,
}

impl BufferCache {
    pub fn new() -> Self {
        Self {
            device: None,
            pages: LruCache::new(NonZeroUsize::new(BUFFER_PAGES_MAX).unwrap()),
            buffer_heads: BTreeMap::new(),
        }
    }

    pub fn set_device(&mut self, device: &Arc<dyn BlockDevice>) {
        self.device = Some(Arc::downgrade(device))
    }

    pub fn device(&self) -> Arc<dyn BlockDevice> {
        self.device.as_ref().unwrap().upgrade().unwrap()
    }

    pub fn read_block(&mut self, block_id: usize, buf: &mut [u8]) {
        // log::error!("block id {block_id}");
        if let Some(buffer_head) = self.buffer_heads.get_mut(&block_id) {
            buffer_head.inc_acc_cnt();
            // log::error!("acc cnt {}", buffer_head.acc_cnt());
            if buffer_head.need_cache() && !buffer_head.has_cached() {
                // log::error!("need cache");
                if let Some(page) = self.pages.get_mut(&block_page_id(block_id)) {
                    // log::error!("has page");
                    BLOCK_DEVICE
                        .get()
                        .unwrap()
                        .base_read_block(block_id, page.block_range(block_id));
                    page.set_buffer_head(buffer_head.clone());
                } else {
                    // log::error!("page init");
                    let mut page = BufferPage::new();
                    BLOCK_DEVICE
                        .get()
                        .unwrap()
                        .base_read_block(block_id, page.block_range(block_id));
                    page.set_buffer_head(buffer_head.clone());
                    self.pages.push(block_page_id(block_id), page);
                };
            }
            if buffer_head.has_cached() {
                // log::error!("cached");
                buffer_head.read_block(buf)
            } else {
                // log::error!("not cached");
                BLOCK_DEVICE.get().unwrap().base_read_block(block_id, buf)
            }
        } else {
            // log::error!("init not cached");
            let buffer_head = BufferHead::new(block_id);
            buffer_head.inc_acc_cnt();
            self.buffer_heads.insert(block_id, Arc::new(buffer_head));
            BLOCK_DEVICE.get().unwrap().base_read_block(block_id, buf)
        }
    }
}

pub fn block_page_id(block_id: usize) -> usize {
    block_id / BUFFERS_IN_PAGE
}

pub fn block_page_offset(block_id: usize) -> usize {
    block_id % BUFFERS_IN_PAGE * BLOCK_SIZE
}

/// A buffer page holds contiguous buffer heads.
pub struct BufferPage {
    pub page: Arc<Page>,
    pub buffer_heads: [Arc<BufferHead>; BUFFERS_IN_PAGE],
}

impl BufferPage {
    pub fn new() -> Self {
        Self {
            page: Arc::new(Page::new()),
            buffer_heads: core::array::from_fn(|_| Arc::new(BufferHead::default())),
        }
    }

    pub fn block_range(&self, block_id: usize) -> &'static mut [u8] {
        let offset = block_id % BUFFERS_IN_PAGE * BLOCK_SIZE;
        self.page.bytes_array_range(offset..offset + BLOCK_SIZE)
    }

    pub fn set_buffer_head(&mut self, buffer_head: Arc<BufferHead>) {
        buffer_head.init(&self.page, block_page_offset(buffer_head.block_id));
        let idx = buffer_head.block_id % BUFFERS_IN_PAGE;
        self.buffer_heads[idx] = buffer_head;
    }
}

impl Drop for BufferPage {
    fn drop(&mut self) {
        // flush back to disk
        todo!()
    }
}

#[derive(Default)]
pub struct BufferHead {
    /// Buffer state.
    bstate: usize,
    /// Block index on the device.
    block_id: usize,
    /// Count of access before cached.
    access_count: AtomicUsize,
    once: Once<BufferHeadOnce>,
}

pub struct BufferHeadOnce {
    /// Page cache which holds the actual buffer data.
    page: Weak<Page>,
    /// Offset in page, aligned with `BLOCK_SIZE`.
    offset: usize,
}

impl BufferHead {
    pub fn new(block_id: usize) -> Self {
        Self {
            bstate: 0,
            block_id,
            access_count: 0.into(),
            once: Once::new(),
        }
    }

    pub fn init(&self, page: &Arc<Page>, offset: usize) {
        self.once.call_once(|| BufferHeadOnce {
            page: Arc::downgrade(page),
            offset,
        });
    }

    pub fn acc_cnt(&self) -> usize {
        self.access_count.load(Ordering::SeqCst)
    }

    pub fn inc_acc_cnt(&self) -> usize {
        self.access_count.fetch_add(1, Ordering::SeqCst)
    }

    pub fn need_cache(&self) -> bool {
        self.acc_cnt() >= BUFFER_NEED_CACHE_CNT
    }

    pub fn has_cached(&self) -> bool {
        self.once.is_completed()
    }

    pub fn page(&self) -> Arc<Page> {
        self.once.get().unwrap().page.upgrade().unwrap()
    }

    pub fn offset(&self) -> usize {
        self.once.get().unwrap().offset
    }

    pub fn read_block(&self, buf: &mut [u8]) {
        let offset = self.offset();
        buf.copy_from_slice(self.page().bytes_array_range(offset..offset + BLOCK_SIZE))
    }
}
