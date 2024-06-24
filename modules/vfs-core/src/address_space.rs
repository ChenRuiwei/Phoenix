use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};

use config::mm::PAGE_MASK;
use page::Page;
use spin::Once;

use crate::{Inode, Mutex};

pub struct AddressSpace {
    /// Map from aligned file offset to page cache.
    pages: Mutex<BTreeMap<usize, Arc<Page>>>,
}

impl AddressSpace {
    pub fn new() -> Self {
        Self {
            pages: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn get_page(&self, offset_aligned: usize) -> Option<Arc<Page>> {
        debug_assert!(is_aligned(offset_aligned));
        self.pages.lock().get(&offset_aligned).cloned()
    }

    pub fn insert_page(&self, offset_aligned: usize, page: Arc<Page>) {
        debug_assert!(is_aligned(offset_aligned));
        self.pages.lock().insert(offset_aligned, page);
    }
}

pub fn is_aligned(offset: usize) -> bool {
    offset & PAGE_MASK == 0
}
