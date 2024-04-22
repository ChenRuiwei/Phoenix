use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::{Arc, Weak},
};

use systype::SysResult;

use crate::{inode::Inode, Mutex, SuperBlock};

pub struct DentryMeta {
    /// Name of this file or directory.
    pub name: String,
    pub super_block: Weak<dyn SuperBlock>,
    /// Inode it points to.
    pub inode: Arc<dyn Inode>,
    /// Parent dentry. `None` if root dentry.
    pub parent: Option<Weak<dyn Dentry>>,

    /// Children dentries.
    // PERF: may be no need to be BTreeMap, since we will look up in hash table
    pub children: Mutex<BTreeMap<String, Arc<dyn Dentry>>>,
}

pub trait Dentry: Send + Sync {
    fn meta(&self) -> &DentryMeta;

    fn set_meta(&self, meta: DentryMeta);
}

impl dyn Dentry {
    pub fn name(&self) -> String {
        self.meta().name.clone()
    }

    pub fn inode(&self) -> Arc<dyn Inode> {
        self.meta().inode.clone()
    }

    // TODO:
    pub fn hash(&self) -> usize {
        todo!()
    }

    pub fn parent(&self) -> Option<Arc<dyn Dentry>> {
        self.meta().parent.as_ref().map(|p| p.upgrade().unwrap())
    }

    /// Insert a child to this dentry and return the dentry of the child.
    // TODO: the args are not clear now
    pub fn insert(
        self: Arc<Self>,
        name: &str,
        child: Arc<dyn Inode>,
    ) -> SysResult<Arc<dyn Dentry>> {
        todo!()
    }

    /// Remove a child from this dentry and return the child.
    pub fn remove(&self, name: &str) -> Option<Arc<dyn Dentry>> {
        self.meta().children.lock().remove(name)
    }

    /// Lookup a dentry in the directory
    fn find(&self, path: &str) -> Option<Arc<dyn Dentry>> {
        todo!()
    }

    /// Get the path of this dentry.
    pub fn path(&self) -> String {
        if let Some(p) = self.parent() {
            let path = String::from("/") + self.name().as_str();
            return p.path() + path.as_str();
        } else {
            String::from("/")
        }
    }
}
