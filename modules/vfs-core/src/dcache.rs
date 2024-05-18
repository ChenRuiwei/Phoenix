use alloc::{string::String, sync::Arc, vec::Vec};

use hashbrown::HashMap;
use spin::Once;

use crate::{Dentry, Mutex};

pub static DCACHE: Once<DentryHashMap> = Once::new();

pub fn dcache() -> &'static DentryHashMap {
    DCACHE.call_once(DentryHashMap::new)
}

pub struct DentryHashMap(Mutex<HashMap<String, DentryBucket>>);

impl DentryHashMap {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    // pub fn get(&self, name: &str) -> Option<&DentryBucket> {
    //     self.0.lock().get(name).clone()
    // }
    //
    // pub fn get_mut(&self, name: &str) -> Option<&mut DentryBucket> {
    //     self.0.lock().get_mut(name).clone()
    // }

    pub fn insert(&self, dentry: Arc<dyn Dentry>) {
        let name = dentry.name_string();
        let mut map = self.0.lock();
        if let Some(v) = map.get_mut(&name) {
            v.insert(dentry)
        } else {
            let mut bucket = DentryBucket::new();
            bucket.insert(dentry);
            map.insert(name, bucket);
        }
    }

    pub fn remove(&self, dentry: &Arc<dyn Dentry>) {
        let name = dentry.name_string();
        let mut map = self.0.lock();
        if let Some(v) = map.get_mut(&name) {
            v.remove(dentry)
        } else {
            log::warn!("[DentryHashMap::remove] not in map")
        }
    }
}

pub struct DentryBucket(Vec<Arc<dyn Dentry>>);

impl DentryBucket {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn insert(&mut self, dentry: Arc<dyn Dentry>) {
        self.0.push(dentry);
    }

    pub fn remove(&mut self, dentry: &Arc<dyn Dentry>) {
        let index = self.0.iter().position(|x| Arc::ptr_eq(x, dentry)).unwrap();
        self.0.remove(index);
    }

    pub fn find_by_parent(&self, parent: &Arc<dyn Dentry>) -> Option<Arc<dyn Dentry>> {
        self.0
            .iter()
            .find(|d| Arc::ptr_eq(parent, &d.parent().unwrap()))
            .cloned()
    }

    pub fn find_by_path(&self, path: &str) -> Option<Arc<dyn Dentry>> {
        self.0.iter().find(|d| d.path() == path).cloned()
    }
}
