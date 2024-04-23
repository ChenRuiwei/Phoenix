use vfs_core::{Dentry, DentryMeta, Inode};

pub struct FatDentry {
    meta: DentryMeta,
}

impl FatDentry {
    pub fn new(meta: DentryMeta) -> Self {
        Self { meta }
    }
}

impl Dentry for FatDentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }
}
