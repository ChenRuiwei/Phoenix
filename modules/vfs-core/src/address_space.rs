use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};

use config::mm::PAGE_MASK;
use memory::page::Page;
use spin::Once;

use crate::{Inode, Mutex};

pub struct AddressSpace {
    inode: Once<Weak<dyn Inode>>,
    /// Map from file offset to page cache.
    pages: Mutex<BTreeMap<usize, Arc<Page>>>,
}

impl AddressSpace {
    pub fn new() -> Self {
        Self {
            inode: Once::new(),
            pages: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn set_inode(&self, inode: Arc<dyn Inode>) {
        self.inode.call_once(|| Arc::downgrade(&inode));
    }

    pub fn get_page(&self, offset: usize) -> Option<Arc<Page>> {
        debug_assert!(is_aligned(offset));
        self.pages.lock().get(&offset).cloned()
    }

    pub fn insert_page(&self, offset: usize, page: Page) {
        debug_assert!(is_aligned(offset));
        self.pages.lock().insert(offset, Arc::new(page));
    }
}

pub fn is_aligned(offset: usize) -> bool {
    offset & PAGE_MASK == 0
}
