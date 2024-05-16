use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::{Arc, Weak},
};
use core::mem::MaybeUninit;

use systype::{SysError, SysResult, SyscallResult};

use crate::{inode::Inode, File, InodeMode, Mutex, SuperBlock};

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
        log::debug!("[Dentry::new] new dentry with name {name}");
        let super_block = Arc::downgrade(&super_block);
        let inode = Mutex::new(None);
        if let Some(parent) = parent {
            Self {
                name: name.to_string(),
                super_block,
                inode,
                parent: Some(Arc::downgrade(&parent)),
                children: Mutex::new(BTreeMap::new()),
            }
        } else {
            Self {
                name: name.to_string(),
                super_block,
                inode,
                parent: None,
                children: Mutex::new(BTreeMap::new()),
            }
        }
    }
}

pub trait Dentry: Send + Sync {
    fn meta(&self) -> &DentryMeta;

    /// Open a file associated with the inode that this dentry points to.
    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>>;

    /// Look up in a directory inode and find file with `name`.
    ///
    /// If the named inode does not exist, a negative dentry will be created as
    /// a child and returned. Returning an error code from this routine must
    /// only be done on a real error.
    fn base_lookup(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>>;

    /// Called by the open(2) and creat(2) system calls. Create an inode for a
    /// dentry in the directory inode.
    ///
    /// If the dentry itself has a negative child with `name`, it will create an
    /// inode for the negative child and return the child.
    fn base_create(self: Arc<Self>, name: &str, mode: InodeMode) -> SysResult<Arc<dyn Dentry>>;

    /// Called by the unlink(2) system call. Delete a file inode in a directory
    /// inode.
    fn base_unlink(self: Arc<Self>, name: &str) -> SyscallResult;

    /// Called by the rmdir(2) system call. Delete a dir inode in a directory
    /// inode.
    fn base_rmdir(self: Arc<Self>, name: &str) -> SyscallResult;

    /// Create a negetive child dentry with `name`.
    fn base_new_child(self: Arc<Self>, _name: &str) -> Arc<dyn Dentry> {
        todo!()
    }

    fn inode(&self) -> SysResult<Arc<dyn Inode>> {
        self.meta()
            .inode
            .lock()
            .as_ref()
            .ok_or(SysError::ENOENT)
            .cloned()
    }

    fn super_block(&self) -> Arc<dyn SuperBlock> {
        self.meta().super_block.upgrade().unwrap()
    }

    fn name_string(&self) -> String {
        self.meta().name.clone()
    }

    fn name(&self) -> &str {
        &self.meta().name
    }

    fn parent(&self) -> Option<Arc<dyn Dentry>> {
        self.meta().parent.as_ref().map(|p| p.upgrade().unwrap())
    }

    fn children(&self) -> BTreeMap<String, Arc<dyn Dentry>> {
        self.meta().children.lock().clone()
    }

    fn get_child(&self, name: &str) -> Option<Arc<dyn Dentry>> {
        self.meta().children.lock().get(name).cloned()
    }

    fn set_inode(&self, inode: Arc<dyn Inode>) {
        if self.meta().inode.lock().is_some() {
            log::warn!("[Dentry::set_inode] replace inode in {:?}", self.name());
        }
        *self.meta().inode.lock() = Some(inode);
    }

    /// Insert a child dentry to this dentry.
    fn insert(&self, child: Arc<dyn Dentry>) -> Option<Arc<dyn Dentry>> {
        self.meta()
            .children
            .lock()
            .insert(child.name_string(), child)
    }

    /// Get the path of this dentry.
    // HACK: code looks ugly and may be has problem
    fn path(&self) -> String {
        if let Some(p) = self.parent() {
            let path = if self.name() == "/" {
                String::from("")
            } else {
                String::from("/") + self.name()
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
    pub fn is_negetive(&self) -> bool {
        self.meta().inode.lock().is_none()
    }

    pub fn clear_inode(&self) {
        *self.meta().inode.lock() = None;
    }

    /// Remove a child from this dentry and return the child.
    pub fn remove(&self, name: &str) -> Option<Arc<dyn Dentry>> {
        self.meta().children.lock().remove(name)
    }

    pub fn open(self: &Arc<Self>) -> SysResult<Arc<dyn File>> {
        self.clone().base_open()
    }

    pub fn lookup(self: &Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        // let hash_key = HashKey::new(self, name)?;
        // if let Some(child) = dcache().get(hash_key) {
        //     log::warn!("[Dentry::lookup] find child in hash");
        //     return Ok(child);
        // }
        let child = self.get_child(name);
        if child.is_some() {
            log::trace!(
                "[Dentry::lookup] lookup {name} in cache in path {}",
                self.path()
            );
            return Ok(child.unwrap());
        }
        log::trace!(
            "[Dentry::lookup] lookup {name} not in cache in path {}",
            self.path()
        );
        self.clone().base_lookup(name)
    }

    pub fn create(self: &Arc<Self>, name: &str, mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        self.clone().base_create(name, mode)
    }

    pub fn unlink(self: &Arc<Self>, name: &str) -> SyscallResult {
        self.clone().base_unlink(name)
    }

    pub fn rmdir(self: &Arc<Self>, name: &str) -> SyscallResult {
        self.clone().base_rmdir(name)
    }

    /// Create a negetive child dentry with `name`.
    pub fn new_child(self: &Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        let child = self.clone().base_new_child(name);
        // dcache().insert(child.clone());
        child
    }

    pub fn get_child_or_create(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        self.get_child(name).unwrap_or_else(|| {
            let new_dentry = self.clone().new_child(name);
            self.insert(new_dentry.clone());
            new_dentry
        })
    }
}

impl<T: Send + Sync + 'static> Dentry for MaybeUninit<T> {
    fn meta(&self) -> &DentryMeta {
        todo!()
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        todo!()
    }

    fn base_lookup(self: Arc<Self>, _name: &str) -> SysResult<Arc<dyn Dentry>> {
        todo!()
    }

    fn base_create(self: Arc<Self>, _name: &str, _mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        todo!()
    }

    fn base_unlink(self: Arc<Self>, _name: &str) -> SyscallResult {
        todo!()
    }

    fn base_rmdir(self: Arc<Self>, _name: &str) -> SyscallResult {
        todo!()
    }

    fn path(&self) -> String {
        String::new()
    }
}
