use alloc::sync::Arc;

use systype::SysError;
use vfs_core::{Dentry, DentryMeta, InodeType, SuperBlock};

use super::inode::SimpleInode;

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
}

impl Dentry for SimpleDentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn arc_open(
        self: alloc::sync::Arc<Self>,
    ) -> systype::SysResult<alloc::sync::Arc<dyn vfs_core::File>> {
        todo!()
    }

    fn arc_lookup(
        self: alloc::sync::Arc<Self>,
        name: &str,
    ) -> systype::SysResult<alloc::sync::Arc<dyn Dentry>> {
        if !self.inode()?.itype().is_dir() {
            return Err(SysError::ENOTDIR);
        }
        let self_clone = self.clone();
        let sub_dentry: Arc<dyn Dentry> = self.get_child(name).unwrap_or_else(|| {
            let sb = self.super_block();
            let new_dentry = SimpleDentry::new(name, sb.clone(), Some(self.clone()));
            self_clone.insert(new_dentry.clone());
            new_dentry
        });
        Ok(sub_dentry)
    }

    fn arc_create(
        self: alloc::sync::Arc<Self>,
        name: &str,
        mode: vfs_core::InodeMode,
    ) -> systype::SysResult<alloc::sync::Arc<dyn Dentry>> {
        let sb = self.super_block();
        let sub_dentry = self
            .get_child(name)
            .unwrap_or_else(|| Self::new(name, sb.clone(), Some(self)));
        let sub_inode = SimpleInode::new(mode, sb, 0);
        sub_dentry.set_inode(sub_inode);
        Ok(sub_dentry)
    }

    fn arc_unlink(self: alloc::sync::Arc<Self>, name: &str) -> systype::SyscallResult {
        todo!()
    }

    fn arc_rmdir(self: alloc::sync::Arc<Self>, name: &str) -> systype::SyscallResult {
        todo!()
    }
}
