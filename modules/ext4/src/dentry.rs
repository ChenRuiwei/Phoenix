use alloc::sync::Arc;

use driver::BlockDevice;
use lwext4_rust::{
    bindings::{EEXIST, O_CREAT, O_TRUNC, O_WRONLY},
    InodeTypes,
};
use systype::{SysError, SysResult};
use vfs_core::{
    Dentry, DentryMeta, DentryState, File, FileSystemType, FileSystemTypeMeta, Inode, InodeMode,
    InodeType, MountFlags, RenameFlags, StatFs, SuperBlock, SuperBlockMeta,
};

use crate::{file::Ext4File, inode::Ext4Inode, LwExt4File};

pub struct Ext4Dentry {
    meta: DentryMeta,
}

impl Ext4Dentry {
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

impl Dentry for Ext4Dentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        let inode = self
            .inode()?
            .downcast_arc::<Ext4Inode>()
            .map_err(|_| SysError::EIO)?;
        Ok(Ext4File::new(self, inode))
    }

    fn base_lookup(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        let sb = self.super_block();
        let inode = self.inode()?;
        let inode = inode
            .downcast_arc::<Ext4Inode>()
            .map_err(|_| SysError::EIO)?;
        let mut file = inode.file.lock();
        let sub_dentry = self.into_dyn().get_child(name).unwrap();
        let fpath = sub_dentry.path();
        if file.check_inode_exist(&fpath, InodeTypes::EXT4_DE_DIR) {
            let new_file = LwExt4File::new(&fpath, InodeTypes::EXT4_DE_DIR);
            let new_inode = Ext4Inode::new(sb, new_file);
            sub_dentry.set_inode(new_inode);
        } else if file.check_inode_exist(&fpath, InodeTypes::EXT4_DE_REG_FILE) {
            let new_file = LwExt4File::new(&fpath, InodeTypes::EXT4_DE_REG_FILE);
            let new_inode = Ext4Inode::new(sb, new_file);
            sub_dentry.set_inode(new_inode);
        }
        Ok(sub_dentry)
    }

    fn base_create(self: Arc<Self>, name: &str, mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        let fpath = self.path() + name;

        log::debug!("[Ext4Dentry::base_create] fpath:{fpath}, mode:{mode:?}");
        let types = match mode.to_type() {
            InodeType::Dir => InodeTypes::EXT4_DE_DIR,
            InodeType::File => InodeTypes::EXT4_DE_REG_FILE,
            _ => unimplemented!(),
        };

        let sb = self.super_block();
        let inode = self
            .inode()?
            .downcast_arc::<Ext4Inode>()
            .map_err(|_| SysError::EIO)?;
        let sub_dentry = self.into_dyn().get_child_or_create(name);
        let mut file = inode.file.lock();
        if file.check_inode_exist(&fpath, types.clone()) {
            return Err(SysError::EEXIST);
        }
        if types == InodeTypes::EXT4_DE_DIR {
            file.dir_mk(&fpath).map_err(SysError::from_i32)?;
        } else {
            file.file_open(&fpath, O_WRONLY | O_CREAT | O_TRUNC)
                .expect("create file failed");
            file.file_close().map_err(SysError::from_i32)?;
        }
        let new_file = LwExt4File::new(&fpath, types);
        let new_inode = Ext4Inode::new(sb, new_file);
        sub_dentry.set_inode(new_inode);
        Ok(sub_dentry)
    }

    fn base_remove(self: Arc<Self>, name: &str) -> SysResult<()> {
        let fpath = self.path() + name;
        let inode = self
            .inode()?
            .downcast_arc::<Ext4Inode>()
            .map_err(|_| SysError::EIO)?;
        let mut file = inode.file.lock();
        if file.check_inode_exist(&fpath, InodeTypes::EXT4_DE_DIR) {
            // Recursive directory remove
            file.dir_rm(&fpath).map(|_v| ()).map_err(SysError::from_i32)
        } else {
            file.file_remove(&fpath)
                .map(|_v| ())
                .map_err(SysError::from_i32)
        }
    }

    fn base_new_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        Self::new(name, self.super_block(), Some(self))
    }

    fn base_rename_to(self: Arc<Self>, new: Arc<dyn Dentry>, flags: RenameFlags) -> SysResult<()> {
        // TODO: lwext4_rust does not support RENAME_EXCHANGE, it remove old path when
        // renaming
        let old_inode = self
            .inode()?
            .downcast_arc::<Ext4Inode>()
            .map_err(|_| SysError::EIO)?;
        let old_itype = self.inode()?.itype();
        if !new.is_negetive() {
            let new_inode = new
                .inode()?
                .downcast_arc::<Ext4Inode>()
                .map_err(|_| SysError::EIO)?;
            let new_itype = new.inode()?.itype();
            if new_itype != old_itype {
                return match (old_itype, new_itype) {
                    (InodeType::File, InodeType::Dir) => Err(SysError::EISDIR),
                    (InodeType::Dir, InodeType::File) => Err(SysError::ENOTDIR),
                    _ => unimplemented!(),
                };
            }
        }
        match old_itype {
            InodeType::Dir => {
                old_inode.file.lock().dir_mv(&self.path(), &new.path());
            }
            InodeType::File => {
                old_inode.file.lock().file_rename(&self.path(), &new.path());
            }
            InodeType::SymLink => todo!(),
            _ => unimplemented!(),
        }
        new.set_inode(self.inode()?);
        if flags.contains(RenameFlags::RENAME_EXCHANGE) {
            self.set_inode(new.inode()?);
        } else {
            self.clear_inode();
        }
        Ok(())
    }
}
