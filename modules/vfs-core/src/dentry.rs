use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::{Arc, Weak},
};
use core::{default, fmt::Error, mem::MaybeUninit, str::FromStr};

use sync::mutex::spin_mutex::SpinMutex;
use systype::{SysError, SysResult, SyscallResult};

use crate::{inode::Inode, File, InodeMode, InodeState, InodeType, Mutex, RenameFlags, SuperBlock};

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
    pub state: Mutex<DentryState>,
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
        Self {
            name: name.to_string(),
            super_block,
            inode,
            parent: parent.map(|p| Arc::downgrade(&p)),
            children: Mutex::new(BTreeMap::new()),
            state: Mutex::new(DentryState::UnInit),
        }
    }
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub enum DentryState {
    /// Either not read from disk or write in memory.
    #[default]
    UnInit,
    Sync,
    Dirty,
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

    /// Called by the unlink(2) system call. Reduce an inode ref count in a
    /// directory inode. Delete the inode when inode ref count is one.
    fn base_unlink(self: Arc<Self>, name: &str) -> SysResult<()>;

    fn base_rename_to(self: Arc<Self>, new: Arc<dyn Dentry>, flags: RenameFlags) -> SysResult<()> {
        Err(SysError::EINVAL)
    }

    fn base_symlink(self: Arc<Self>, name: &str, target: &str) -> SysResult<()> {
        Err(SysError::EINVAL)
    }

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

    fn remove_child(&self, name: &str) -> Option<Arc<dyn Dentry>> {
        self.meta().children.lock().remove(name)
    }

    fn set_inode(&self, inode: Arc<dyn Inode>) {
        if self.meta().inode.lock().is_some() {
            log::warn!("[Dentry::set_inode] replace inode in {:?}", self.name());
        }
        *self.meta().inode.lock() = Some(inode);
    }

    fn clear_inode(&self) {
        *self.meta().inode.lock() = None;
    }

    /// Insert a child dentry to this dentry.
    fn insert(&self, child: Arc<dyn Dentry>) -> Option<Arc<dyn Dentry>> {
        self.meta()
            .children
            .lock()
            .insert(child.name_string(), child)
    }

    fn set_state(&self, state: DentryState) {
        *self.meta().state.lock() = state;
    }

    /// Get the path of this dentry.
    fn path(&self) -> String {
        if let Some(p) = self.parent() {
            let p_path = p.path();
            if p_path == "/" {
                p_path + self.name()
            } else {
                p_path + "/" + self.name()
            }
        } else {
            String::from("/")
        }
    }
}

impl dyn Dentry {
    pub fn state(&self) -> DentryState {
        *self.meta().state.lock()
    }

    pub fn is_negetive(&self) -> bool {
        self.meta().inode.lock().is_none()
    }

    pub fn open(self: &Arc<Self>) -> SysResult<Arc<dyn File>> {
        self.clone().base_open()
    }

    pub fn lookup(self: &Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        if !self.inode()?.itype().is_dir() {
            return Err(SysError::ENOTDIR);
        }
        let child = self.get_child_or_create(name);
        if child.state() == DentryState::UnInit {
            log::trace!(
                "[Dentry::lookup] lookup {name} not in cache in path {}",
                self.path()
            );
            self.clone().base_lookup(name)?;
            child.set_state(DentryState::Sync);
            return Ok(child);
        }
        Ok(child)
    }

    pub fn create(self: &Arc<Self>, name: &str, mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        if !self.inode()?.itype().is_dir() {
            return Err(SysError::ENOTDIR);
        }
        let child = self.get_child_or_create(name);
        if child.is_negetive() {
            self.clone().base_create(name, mode)?;
        }
        Ok(child)
    }

    pub fn unlink(self: &Arc<Self>, name: &str) -> SysResult<()> {
        if !self.inode()?.itype().is_dir() {
            return Err(SysError::ENOTDIR);
        }
        let sub_dentry = self.get_child(name).ok_or(SysError::ENOENT)?;
        sub_dentry.inode()?.set_state(InodeState::Removed);
        self.clone().base_unlink(name)?;
        sub_dentry.clear_inode();
        Ok(())
    }

    pub fn rename_to(self: &Arc<Self>, new: &Arc<Self>, flags: RenameFlags) -> SysResult<()> {
        if flags.contains(RenameFlags::RENAME_EXCHANGE)
            && (flags.contains(RenameFlags::RENAME_NOREPLACE)
                || flags.contains(RenameFlags::RENAME_WHITEOUT))
        {
            return Err(SysError::EINVAL);
        }
        if new.is_descendant_of(self) {
            return Err(SysError::EINVAL);
        }

        if new.is_negetive() && flags.contains(RenameFlags::RENAME_EXCHANGE) {
            return Err(SysError::ENOENT);
        } else if flags.contains(RenameFlags::RENAME_NOREPLACE) {
            return Err(SysError::EEXIST);
        }
        self.clone().base_rename_to(new.clone(), flags)
    }

    pub fn symlink(self: &Arc<Self>, name: &str, target: &str) -> SysResult<()> {
        if !self.inode()?.itype().is_dir() {
            return Err(SysError::ENOTDIR);
        }
        let child = self.get_child_or_create(name);
        if child.is_negetive() {
            self.clone().base_symlink(name, target)
        } else {
            Err(SysError::EEXIST)
        }
    }

    /// Create a negetive child dentry with `name`.
    pub fn new_child(self: &Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        let child = self.clone().base_new_child(name);
        child
    }

    pub fn get_child_or_create(self: &Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        self.get_child(name).unwrap_or_else(|| {
            let new_dentry = self.new_child(name);
            self.insert(new_dentry.clone());
            new_dentry
        })
    }

    pub fn is_descendant_of(self: &Arc<Self>, dir: &Arc<Self>) -> bool {
        let mut parent_opt = self.parent();
        while let Some(parent) = parent_opt {
            if Arc::ptr_eq(self, dir) {
                return true;
            }
            parent_opt = parent.parent();
        }
        false
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

    fn base_unlink(self: Arc<Self>, _name: &str) -> SysResult<()> {
        todo!()
    }

    fn path(&self) -> String {
        "no path".to_string()
    }
}
