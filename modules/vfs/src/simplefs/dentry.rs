use alloc::sync::Arc;

use systype::{SysError, SysResult};
use vfs_core::{Dentry, DentryMeta, File, Inode, InodeMode, InodeType, SuperBlock};

use super::{
    file::{SimpleDirFile, SimpleFileFile},
    inode::{SimpleDirInode, SimpleFileInode},
};

pub struct SimpleDentry {
    meta: DentryMeta,
}

impl SimpleDentry {
    pub fn new(
        name: &str,
        super_block: Arc<dyn SuperBlock>,
        parent: Option<Arc<dyn Dentry>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new(name, super_block, parent),
        })
    }

    pub fn into_dyn(self: Arc<Self>) -> Arc<dyn Dentry> {
        self.clone()
    }
}

impl Dentry for SimpleDentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        let inode = self.inode()?;
        match inode.itype() {
            InodeType::Dir => Ok(SimpleDirFile::new(self.clone(), inode)),
            InodeType::File => Ok(SimpleFileFile::new(self.clone(), inode)),
            InodeType::Socket => Ok(SimpleFileFile::new(self.clone(), inode)),
            _ => unreachable!(),
        }
    }

    fn base_lookup(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        let sub_dentry = self.into_dyn().get_child_or_create(name);
        Ok(sub_dentry)
    }

    fn base_create(self: Arc<Self>, name: &str, mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        let sb = self.super_block();
        let sub_dentry = self.into_dyn().get_child_or_create(name);
        let sub_inode: Arc<dyn Inode> = match mode.to_type() {
            InodeType::Dir => SimpleDirInode::new(mode, sb, 0),
            InodeType::File => SimpleFileInode::new(mode, sb, 0),
            _ => return Err(SysError::EPERM),
        };
        sub_dentry.set_inode(sub_inode);
        Ok(sub_dentry)
    }

    fn base_unlink(self: Arc<Self>, name: &str) -> SysResult<()> {
        self.remove_child(name).ok_or(SysError::ENOENT).map(|_| ())
    }

    fn base_new_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        Self::new(name, self.super_block(), Some(self))
    }
}
