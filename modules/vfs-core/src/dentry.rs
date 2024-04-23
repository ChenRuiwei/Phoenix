use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::{Arc, Weak},
};

use systype::SysResult;

use crate::{inode::Inode, super_block, InodeMode, Mutex, SuperBlock};

pub struct DentryMeta {
    /// Name of this file or directory.
    pub name: String,
    pub super_block: Weak<dyn SuperBlock>,
    /// Inode it points to.
    pub inode: Arc<dyn Inode>,
    /// Parent dentry. `None` if root dentry.
    pub parent: Option<Weak<dyn Dentry>>,

    /// Children dentries. Key value pair is <name, dentry>.
    // PERF: may be no need to be BTreeMap, since we will look up in hash table
    pub children: Mutex<BTreeMap<String, Arc<dyn Dentry>>>,
}

impl DentryMeta {
    pub fn new(
        name: &str,
        super_block: Arc<dyn SuperBlock>,
        inode: Arc<dyn Inode>,
        parent: Option<Weak<dyn Dentry>>,
    ) -> Self {
        Self {
            name: name.to_string(),
            super_block: Arc::downgrade(&super_block),
            inode,
            parent,
            children: Mutex::new(BTreeMap::new()),
        }
    }
}

pub trait Dentry: Send + Sync {
    fn meta(&self) -> &DentryMeta;
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

    /// Lookup a dentry with `name` in the directory.
    fn find(&self, name: &str) -> Option<Arc<dyn Dentry>> {
        let meta = self.meta();
        let mode = meta.inode.mode();
        match mode {
            InodeMode::Dir => meta.children.lock().get(name).map(|item| item.clone()),
            _ => None,
        }
    }

    /// Get the path of this dentry.
    fn path(&self) -> String {
        if let Some(p) = self.parent() {
            let path = if self.name() == "/" {
                String::from("")
            } else {
                String::from("/") + self.name().as_str()
            };
            let parent_name = p.name();
            return if parent_name == "/" {
                if p.parent().is_some() {
                    // p is a mount point
                    p.parent().unwrap().path() + path.as_str()
                } else {
                    path
                }
            } else {
                // p is not root
                p.path() + path.as_str()
            };
        } else {
            log::warn!("dentry has no parent");
            String::from("/")
        }
    }
}
