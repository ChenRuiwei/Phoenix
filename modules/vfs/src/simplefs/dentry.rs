use alloc::sync::Arc;

use systype::{SysError, SysResult};
use vfs_core::{Dentry, DentryMeta, File, InodeMode, InodeType, SuperBlock};

use super::{file::SimpleDirFile, inode::SimpleInode};

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
            _ => unreachable!(),
        }
    }

    fn base_lookup(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        if !self.inode()?.itype().is_dir() {
            return Err(SysError::ENOTDIR);
        }
        let sub_dentry = self.into_dyn().get_child_or_create(name);
        Ok(sub_dentry)
    }

    fn base_create(self: Arc<Self>, name: &str, mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        let sb = self.super_block();
        let sub_dentry = self.into_dyn().get_child_or_create(name);
        let sub_inode = SimpleInode::new(mode, sb, 0);
        sub_dentry.set_inode(sub_inode);
        Ok(sub_dentry)
    }

    fn base_unlink(self: Arc<Self>, _name: &str) -> systype::SyscallResult {
        todo!()
    }

    fn base_rmdir(self: alloc::sync::Arc<Self>, _name: &str) -> systype::SyscallResult {
        todo!()
    }

    fn base_new_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        Self::new(name, self.super_block(), Some(self))
    }
}
