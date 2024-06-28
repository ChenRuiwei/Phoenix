use alloc::{
    collections::{BTreeMap, LinkedList},
    sync::{Arc, Weak},
};

use config::mm::is_page_aligned;
use device_core::BlockDevice;
use sync::mutex::SpinNoIrqLock;

use crate::{BufferHeadAdapter, Page};

pub struct PageCache {
    /// Map from aligned file offset to page cache.
    pages: SpinNoIrqLock<BTreeMap<usize, Arc<Page>>>,
}

impl PageCache {
    pub fn new() -> Self {
        Self {
            pages: SpinNoIrqLock::new(BTreeMap::new()),
        }
    }

    pub fn get_page(&self, offset_aligned: usize) -> Option<Arc<Page>> {
        debug_assert!(is_page_aligned(offset_aligned));
        self.pages.lock().get(&offset_aligned).cloned()
    }

    pub fn insert_page(&self, offset_aligned: usize, page: Arc<Page>) {
        debug_assert!(is_page_aligned(offset_aligned));
        self.pages.lock().insert(offset_aligned, page);
    }

    pub fn flush(&self) {
        for page in self.pages.lock().values() {
            page.flush()
        }
    }
}
