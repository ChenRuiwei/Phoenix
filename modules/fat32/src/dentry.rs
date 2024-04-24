use alloc::sync::{Arc, Weak};

use systype::SysError;
use vfs_core::{Dentry, DentryMeta, Inode, SuperBlock};

use crate::{
    as_sys_err,
    file::FatFileFile,
    inode::{dir::FatDirInode, file::FatFileInode},
};

pub struct FatDentry {
    meta: DentryMeta,
}

impl FatDentry {
    pub fn new(
        name: &str,
        super_block: Arc<dyn SuperBlock>,
        parent: Option<Arc<dyn Dentry>>,
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

    fn open(&self) -> systype::SysResult<Arc<dyn vfs_core::File>> {
        if let Ok(inode) = self.inode().downcast_arc::<FatFileInode>() {
            Ok(FatFileFile::new(self.path(), inode))
        } else {
            Err(SysError::EISDIR)
        }
    }

    fn arc_lookup(self: Arc<Self>, name: &str) -> systype::SysResult<Arc<dyn Dentry>> {
        let sb = self.meta().super_block.upgrade().unwrap();
        // TODO: if children already exists
        let new_dentry: Arc<dyn Dentry> = FatDentry::new(&name, sb.clone(), Some(self.clone()));
        let inode = self
            .inode()
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;
        let dir = inode.dir.lock();
        let find = dir.iter().find(|e| {
            let entry = e.as_ref().unwrap();
            let e_name = entry.file_name();
            name == e_name
        });
        if let Some(find) = find {
            let entry = find.map_err(as_sys_err)?;
            if entry.is_dir() {
                let new_dir = dir.open_dir(&name).map_err(as_sys_err)?;
                drop(dir);
                let inode = FatDirInode::new(sb, new_dir);
                new_dentry.set_inode(inode);
            } else {
                let file = dir.open_file(&name).map_err(as_sys_err)?;
                drop(dir);
                let inode = FatFileInode::new(sb, file);
                new_dentry.set_inode(inode);
            }
        } else {
            new_dentry.clear_inode();
        }
        self.insert(name, new_dentry.clone());
        Ok(new_dentry)
    }
}
