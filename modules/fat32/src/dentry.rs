use alloc::sync::{Arc, Weak};

use systype::SysError;
use vfs_core::{dcache, Dentry, DentryMeta, Inode, InodeType, SuperBlock};

use crate::{
    as_sys_err,
    file::{FatDirFile, FatFileFile},
    inode::{self, dir::FatDirInode, file::FatFileInode},
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
        let dentry = Arc::new(Self {
            meta: DentryMeta::new(name, super_block, parent),
        });
        dentry
    }

    pub fn into_dyn(self: Arc<Self>) -> Arc<dyn Dentry> {
        self.clone()
    }
}

impl Dentry for FatDentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> systype::SysResult<Arc<dyn vfs_core::File>> {
        let inode = self.inode()?;
        match inode.itype() {
            InodeType::File => {
                let inode = inode
                    .downcast_arc::<FatFileInode>()
                    .map_err(|_| SysError::EIO)?;
                Ok(FatFileFile::new(self.clone(), inode))
            }
            InodeType::Dir => {
                let inode = inode
                    .downcast_arc::<FatDirInode>()
                    .map_err(|_| SysError::EIO)?;
                Ok(FatDirFile::new(self.clone(), inode))
            }
            _ => Err(SysError::EPERM),
        }
    }

    fn base_lookup(self: Arc<Self>, name: &str) -> systype::SysResult<Arc<dyn Dentry>> {
        let sb = self.super_block();
        let self_clone = self.clone();
        let inode = self
            .inode()?
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;
        let find = inode.dir.lock().iter().find(|e| {
            let entry = e.as_ref().unwrap();
            let e_name = entry.file_name();
            name == e_name
        });
        let sub_dentry = self.into_dyn().get_child_or_create(name);
        if let Some(find) = find {
            log::debug!("[FatDentry::base_lookup] find name {name}");
            let entry = find.map_err(as_sys_err)?;
            let new_inode: Arc<dyn Inode> = if entry.is_dir() {
                let new_dir = entry.to_dir();
                FatDirInode::new(sb, new_dir)
            } else {
                let new_file = entry.to_file();
                FatFileInode::new(sb, new_file)
            };
            sub_dentry.set_inode(new_inode);
        } else {
            log::warn!("[FatDentry::base_lookup] name {name} does not exist");
        }
        Ok(sub_dentry)
    }

    fn base_create(
        self: Arc<Self>,
        name: &str,
        mode: vfs_core::InodeMode,
    ) -> systype::SysResult<Arc<dyn Dentry>> {
        log::trace!("[FatDentry::base_create] create name {name}, mode {mode:?}");
        let sb = self.super_block();
        let inode = self
            .inode()?
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;
        let sub_dentry = self.into_dyn().get_child_or_create(name);
        match mode.to_type() {
            InodeType::Dir => {
                let new_dir = inode.dir.lock().create_dir(name).map_err(as_sys_err)?;
                let new_inode = FatDirInode::new(sb.clone(), new_dir);
                sub_dentry.set_inode(new_inode);
                Ok(sub_dentry)
            }
            InodeType::File => {
                let new_file = inode.dir.lock().create_file(name).map_err(as_sys_err)?;
                let new_inode = FatFileInode::new(sb.clone(), new_file);
                sub_dentry.set_inode(new_inode);
                Ok(sub_dentry)
            }
            _ => {
                log::warn!("[FatDentry::base_create] not supported mode {mode:?}");
                Err(SysError::EIO)
            }
        }
    }

    fn base_unlink(self: Arc<Self>, name: &str) -> systype::SyscallResult {
        let inode = self
            .inode()?
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;
        let sub_dentry = self.get_child(name).ok_or(SysError::ENOENT)?;
        if sub_dentry.inode()?.itype().is_dir() {
            return Err(SysError::EISDIR);
        }
        sub_dentry.clear_inode();
        inode.dir.lock().remove(name).map_err(as_sys_err)?;
        Ok(0)
    }

    fn base_rmdir(self: Arc<Self>, name: &str) -> systype::SyscallResult {
        let inode = self
            .inode()?
            .downcast_arc::<FatDirInode>()
            .map_err(|_| SysError::ENOTDIR)?;
        let sub_dentry = self.get_child(name).ok_or(SysError::ENOENT)?;
        if !sub_dentry.inode()?.itype().is_dir() {
            return Err(SysError::ENOTDIR);
        }
        sub_dentry.clear_inode();
        inode.dir.lock().remove(name).map_err(as_sys_err)?;
        Ok(0)
    }

    fn base_new_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        Self::new(name, self.super_block(), Some(self))
    }
}
