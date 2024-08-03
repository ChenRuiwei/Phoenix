use alloc::{string::String, sync::Arc};

use hashbrown::{HashMap, HashTable};
use spin::Once;

use crate::{Dentry, HashKey, Mutex};

pub static DCACHE: Once<DentryHashMap> = Once::new();

pub fn dcache() -> &'static DentryHashMap {
    DCACHE.call_once(DentryHashMap::new)
}

pub struct DentryHashMap(Mutex<HashMap<HashKey, Arc<dyn Dentry>>>);

impl DentryHashMap {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    pub fn get(&self, key: HashKey) -> Option<Arc<dyn Dentry>> {
        self.0.lock().get(&key).cloned()
    }

    pub fn insert(&self, dentry: Arc<dyn Dentry>) {
        self.0.lock().insert(dentry.hash(), dentry);
    }

    pub fn remove(&self, key: HashKey) -> Option<Arc<dyn Dentry>> {
        self.0.lock().remove(&key)
    }
}
