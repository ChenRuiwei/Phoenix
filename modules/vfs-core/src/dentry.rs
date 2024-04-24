use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::{Arc, Weak},
};

use spin::Once;
use systype::SysResult;

use crate::{inode::Inode, super_block, File, InodeMode, Mutex, SuperBlock};

pub struct DentryMeta {
    /// Name of this file or directory.
    pub name: String,
    pub super_block: Weak<dyn SuperBlock>,
    /// Parent dentry. `None` if root dentry.
    pub parent: Option<Weak<dyn Dentry>>,

    /// Inode it points to. May be `None`, which is called negative dentry.
    pub inode: Mutex<Option<Arc<dyn Inode>>>,
    /// Children dentries. Key value pair is <name, dentry>.
    // PERF: may be no need to be BTreeMap, since we will look up in hash table
    pub children: Mutex<BTreeMap<String, Arc<dyn Dentry>>>,
}

impl DentryMeta {
    pub fn new(
        name: &str,
        super_block: Arc<dyn SuperBlock>,
        parent: Option<Arc<dyn Dentry>>,
    ) -> Self {
        Self {
            name: name.to_string(),
            super_block: Arc::downgrade(&super_block),
            inode: Mutex::new(None),
            parent: parent.map(|p| Arc::downgrade(&p)),
            children: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn new_with_inode(
        name: &str,
        super_block: Arc<dyn SuperBlock>,
        inode: Arc<dyn Inode>,
        parent: Option<Weak<dyn Dentry>>,
    ) -> Self {
        Self {
            name: name.to_string(),
            super_block: Arc::downgrade(&super_block),
            parent,
            inode: Mutex::new(Some(inode)),
            children: Mutex::new(BTreeMap::new()),
        }
    }
}

pub trait Dentry: Send + Sync {
    fn meta(&self) -> &DentryMeta;

    fn inode(&self) -> Arc<dyn Inode> {
        self.meta().inode.lock().as_ref().unwrap().clone()
    }

    /// Open a file associated with the inode that this dentry points to.
    fn open(&self) -> SysResult<Arc<dyn File>>;

    /// Look up in a directory inode and find file with `name`.
    fn arc_lookup(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>>;

    fn super_block(&self) -> Arc<dyn SuperBlock> {
        self.meta().super_block.upgrade().unwrap()
    }

    fn name(&self) -> String {
        self.meta().name.clone()
    }

    fn parent(&self) -> Option<Arc<dyn Dentry>> {
        self.meta().parent.as_ref().map(|p| p.upgrade().unwrap())
    }

    /// Insert a child dentry to this dentry.
    fn insert(self: Arc<Self>, name: &str, child: Arc<dyn Dentry>) {
        self.meta().children.lock().insert(name.to_string(), child);
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

impl dyn Dentry {
    pub fn set_inode(&self, inode: Arc<dyn Inode>) {
        debug_assert!(self.meta().inode.lock().is_none());
        *self.meta().inode.lock() = Some(inode);
    }

    pub fn clear_inode(&self) {
        *self.meta().inode.lock() = None;
    }

    // TODO:
    pub fn hash(&self) -> usize {
        todo!()
    }

    /// Remove a child from this dentry and return the child.
    pub fn remove(&self, name: &str) -> Option<Arc<dyn Dentry>> {
        self.meta().children.lock().remove(name)
    }

    /// Lookup a dentry with `name` in the directory.
    pub fn find(&self, name: &str) -> Option<Arc<dyn Dentry>> {
        let meta = self.meta();
        let mode = self.inode().mode();
        match mode {
            InodeMode::Dir => meta.children.lock().get(name).map(|item| item.clone()),
            _ => None,
        }
    }

    pub fn lookup(self: &Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        self.clone().arc_lookup(name)
    }
}

impl Dentry for DentryMeta {
    fn meta(&self) -> &DentryMeta {
        self
    }

    fn open(&self) -> SysResult<Arc<dyn File>> {
        todo!()
    }

    fn arc_lookup(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        todo!()
    }
}
