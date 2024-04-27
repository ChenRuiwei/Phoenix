use alloc::sync::{Arc, Weak};

use systype::SysError;
use vfs_core::{Dentry, DentryMeta, Inode, InodeType, SuperBlock};

use crate::{
    as_sys_err,
    file::{FatDirFile, FatFileFile},
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

    pub fn into_dyn(self: Arc<Self>) -> Arc<dyn Dentry> {
        self.clone()
    }
}

impl Dentry for FatDentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn arc_open(self: Arc<Self>) -> systype::SysResult<Arc<dyn vfs_core::File>> {
        match self.inode()?.itype() {
            InodeType::File => {
                let inode = self
                    .inode()?
                    .downcast_arc::<FatFileInode>()
                    .map_err(|_| SysError::EIO)?;
                Ok(FatFileFile::new(self.clone(), inode))
            }
            InodeType::Dir => {
                let inode = self
                    .inode()?
                    .downcast_arc::<FatDirInode>()
                    .map_err(|_| SysError::EIO)?;
                Ok(FatDirFile::new(self.clone(), inode))
            }
            _ => Err(SysError::EPERM),
        }
    }

    fn arc_lookup(self: Arc<Self>, name: &str) -> systype::SysResult<Arc<dyn Dentry>> {
        let sb = self.super_block();
        // TODO: if children already exists
        let new_dentry: Arc<dyn Dentry> = FatDentry::new(&name, sb.clone(), Some(self.clone()));
        let inode = self
            .inode()?
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
                let new_inode = FatDirInode::new(sb, new_dir);
                new_dentry.set_inode(new_inode);
            } else {
                let file = dir.open_file(&name).map_err(as_sys_err)?;
                drop(dir);
                let new_inode = FatFileInode::new(sb, file);
                new_dentry.set_inode(new_inode);
            }
        } else {
            new_dentry.clear_inode();
        }
        self.insert(name, new_dentry.clone());
        Ok(new_dentry)
    }

    fn arc_create(
        self: Arc<Self>,
        name: &str,
        mode: vfs_core::InodeMode,
    ) -> systype::SysResult<Arc<dyn Dentry>> {
        log::trace!("[FatDentry::arc_create]");
        let sb = self.super_block();
        let inode = self
            .inode()?
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;
        let sub_dentry = self
            .find_child(name)
            .unwrap_or_else(|| Self::new(name, sb.clone(), Some(self)));
        match mode.to_type() {
            InodeType::Dir => {
                let new_dir = inode.dir.lock().create_dir(&name).map_err(as_sys_err)?;
                let new_inode = FatDirInode::new(sb.clone(), new_dir);
                sub_dentry.set_inode(inode);
                // let new_dentry = Self::new_with_inode(
                //     name,
                //     sb,
                //     new_inode,
                //     Some(Arc::downgrade(&self.into_dyn())),
                // );
                Ok((sub_dentry))
            }
            InodeType::File => {
                let new_file = inode.dir.lock().create_file(&name).map_err(as_sys_err)?;
                let new_inode = FatFileInode::new(sb.clone(), new_file);
                sub_dentry.set_inode(inode);
                // let new_dentry = Self::new_with_inode(
                //     name,
                //     sb,
                //     new_inode,
                //     Some(Arc::downgrade(&self.into_dyn())),
                // );
                Ok((sub_dentry))
            }
            _ => {
                log::warn!("[FatDentry::arc_create] not supported mode {mode:?}");
                Err(SysError::EIO)
            }
        }
    }
}
