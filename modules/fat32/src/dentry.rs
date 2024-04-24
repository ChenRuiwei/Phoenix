use alloc::sync::{Arc, Weak};

use vfs_core::{Dentry, DentryMeta, Inode, SuperBlock};

pub struct FatDentry {
    meta: DentryMeta,
}

impl FatDentry {
    pub fn new(
        name: &str,
        super_block: Arc<dyn SuperBlock>,
        parent: Option<Weak<dyn Dentry>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new(name, super_block, parent),
        })
    }

    pub fn new_with_inode(
        name: &str,
        super_block: Arc<dyn SuperBlock>,
        inode: Arc<dyn Inode>,
        parent: Option<Weak<dyn Dentry>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new_with_inode(name, super_block, inode, parent),
        })
    }
}

impl Dentry for FatDentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }
}
