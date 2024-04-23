use alloc::sync::Arc;

use vfs_core::{Dentry, DentryMeta, Inode};

pub struct FatDentry {
    meta: DentryMeta,
}

impl FatDentry {
    pub fn new(meta: DentryMeta) -> Arc<Self> {
        Arc::new(Self { meta })
    }
}

impl Dentry for FatDentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }
}
