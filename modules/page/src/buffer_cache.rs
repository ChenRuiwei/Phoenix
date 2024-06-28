use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    default,
    num::NonZeroUsize,
    sync::atomic::{AtomicUsize, Ordering},
};

use config::{
    board::BLOCK_SIZE,
    mm::{
        block_page_id, is_block_aligned, BUFFER_NEED_CACHE_CNT, MAX_BUFFERS_PER_PAGE,
        MAX_BUFFER_PAGES, PAGE_SIZE,
    },
};
use device_core::BlockDevice;
use intrusive_collections::{intrusive_adapter, LinkedListAtomicLink};
use lru::LruCache;
use spin::Once;
use sync::mutex::SpinNoIrqLock;

use crate::Page;

pub struct BufferCache {
    device: Option<Weak<dyn BlockDevice>>,
    /// Block page id to `Page`.
    // NOTE: These `Page`s are pages without file, only exist for caching pure
    // block data.
    pages: LruCache<usize, Arc<Page>>,
    /// Block idx to `BufferHead`.
    // NOTE: Stores all accesses to block device. Some of them will be attached
    // to pages above, while others with file related will be attached to pages
    // stored in address space.
    buffer_heads: BTreeMap<usize, Arc<BufferHead>>,
}

impl BufferCache {
    pub fn new() -> Self {
        Self {
            device: None,
            pages: LruCache::new(NonZeroUsize::new(MAX_BUFFER_PAGES).unwrap()),
            buffer_heads: BTreeMap::new(),
        }
    }

    pub fn init_device(&mut self, device: Arc<dyn BlockDevice>) {
        self.device = Some(Arc::downgrade(&device))
    }

    pub fn device(&self) -> Arc<dyn BlockDevice> {
        self.device.as_ref().unwrap().upgrade().unwrap()
    }

    pub fn read_block(&mut self, block_id: usize, buf: &mut [u8]) {
        let buffer_head = self.get_buffer_head_from_disk(block_id);
        if buffer_head.has_cached() {
            buffer_head.read_block(buf)
        } else {
            self.device().base_read_block(block_id, buf)
        }
    }

    pub fn write_block(&mut self, block_id: usize, buf: &[u8]) {
        let buffer_head = self.get_buffer_head_from_disk(block_id);
        if buffer_head.has_cached() {
            buffer_head.write_block(buf)
        } else {
            self.device().base_write_block(block_id, buf)
        }
    }

    pub fn get_buffer_head_from_disk(&mut self, block_id: usize) -> Arc<BufferHead> {
        // log::error!("block id {block_id}");
        let device = self.device();
        if let Some(buffer_head) = self.buffer_heads.get_mut(&block_id) {
            buffer_head.inc_acc_cnt();
            // log::error!("acc cnt {}", buffer_head.acc_cnt());
            if buffer_head.need_cache() && !buffer_head.has_cached() {
                // log::error!("need cache");
                if let Some(page) = self.pages.get_mut(&block_page_id(block_id)) {
                    // log::error!("has page");
                    device.base_read_block(block_id, page.block_bytes_array(block_id));
                    page.insert_buffer_head(buffer_head.clone());
                } else {
                    log::info!("buffer page init");
                    let mut page = Page::new_file(&device);
                    device.base_read_block(block_id, page.block_bytes_array(block_id));
                    page.insert_buffer_head(buffer_head.clone());
                    self.pages.push(block_page_id(block_id), page);
                };
            }
            buffer_head.clone()
        } else {
            log::trace!(
                "[BufferCache::get_buffer_head_from_disk] init buffer_head for blk idx {block_id}"
            );
            let buffer_head = BufferHead::new_arc(block_id);
            buffer_head.inc_acc_cnt();
            self.buffer_heads.insert(block_id, buffer_head.clone());
            buffer_head
        }
    }

    pub fn get_buffer_head(&self, block_id: usize) -> Arc<BufferHead> {
        self.buffer_heads.get(&block_id).cloned().unwrap()
    }
}

pub struct BufferHead {
    /// Block index on the device.
    block_id: usize,
    page_link: LinkedListAtomicLink,
    inner: SpinNoIrqLock<BufferHeadInner>,
}

intrusive_adapter!(pub BufferHeadAdapter = Arc<BufferHead>: BufferHead { page_link: LinkedListAtomicLink });

pub struct BufferHeadInner {
    /// Count of access before cached.
    acc_cnt: usize,
    /// Buffer state.
    bstate: BufferState,
    /// Page cache which holds the actual buffer data.
    page: Weak<Page>,
    /// Offset in page, aligned with `BLOCK_SIZE`.
    offset: usize,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferState {
    #[default]
    UnInit,
    Sync,
    Dirty,
}

impl BufferHead {
    pub fn new(block_id: usize) -> Self {
        Self {
            block_id,
            page_link: LinkedListAtomicLink::new(),
            inner: SpinNoIrqLock::new(BufferHeadInner {
                acc_cnt: 0,
                bstate: BufferState::UnInit,
                page: Weak::new(),
                offset: 0,
            }),
        }
    }

    pub fn new_arc(block_id: usize) -> Arc<Self> {
        Arc::new(Self::new(block_id))
    }

    pub fn init(&self, page: &Arc<Page>, offset: usize) {
        if self.has_cached() {
            log::error!(
                "block id {} already cached, with acc_cnt {}",
                self.block_id,
                self.acc_cnt()
            );
        }
        debug_assert!(is_block_aligned(offset) && offset < PAGE_SIZE);
        let mut inner = self.inner.lock();
        inner.bstate = BufferState::Sync;
        inner.page = Arc::downgrade(page);
        inner.offset = offset;
    }

    pub fn reset(&self) {
        let mut inner = self.inner.lock();
        inner.acc_cnt = 0;
        inner.bstate = BufferState::UnInit;
        inner.page = Weak::new();
        inner.offset = 0;
    }

    pub fn block_id(&self) -> usize {
        self.block_id
    }

    pub fn acc_cnt(&self) -> usize {
        self.inner.lock().acc_cnt
    }

    pub fn inc_acc_cnt(&self) {
        self.inner.lock().acc_cnt += 1
    }

    pub fn need_cache(&self) -> bool {
        self.acc_cnt() >= BUFFER_NEED_CACHE_CNT
    }

    pub fn bstate(&self) -> BufferState {
        self.inner.lock().bstate
    }

    pub fn set_bstate(&self, bstate: BufferState) {
        self.inner.lock().bstate = bstate
    }

    pub fn page(&self) -> Arc<Page> {
        self.inner.lock().page.upgrade().unwrap()
    }

    pub fn offset(&self) -> usize {
        debug_assert!(self.has_cached());
        self.inner.lock().offset
    }

    pub fn has_cached(&self) -> bool {
        self.inner.lock().bstate != BufferState::UnInit
    }

    pub fn bytes_array(&self) -> &'static mut [u8] {
        let offset = self.offset();
        self.page().bytes_array_range(offset..offset + BLOCK_SIZE)
    }

    pub fn read_block(&self, buf: &mut [u8]) {
        buf.copy_from_slice(self.bytes_array())
    }

    pub fn write_block(&self, buf: &[u8]) {
        self.bytes_array().copy_from_slice(buf);
        self.set_bstate(BufferState::Dirty)
    }
}
