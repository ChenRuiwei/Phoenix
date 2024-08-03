use alloc::{
    ffi::CString,
    sync::Arc,
    vec::{self, Vec},
};
use core::fmt::Error;

use lwext4_rust::{
    bindings::EEXIST, lwext4_check_inode_exist, lwext4_mvdir, lwext4_mvfile, lwext4_rmdir,
    lwext4_rmfile, InodeTypes,
};
use systype::{SysError, SysResult};
use vfs_core::{
    Dentry, DentryMeta, DentryState, File, FileSystemType, FileSystemTypeMeta, Inode, InodeMode,
    InodeType, MountFlags, OpenFlags, Path, RenameFlags, StatFs, SuperBlock, SuperBlockMeta,
};

use crate::{
    file::Ext4FileFile, inode::Ext4FileInode, Ext4DirFile, Ext4DirInode, Ext4LinkFile,
    Ext4LinkInode, LwExt4Dir, LwExt4File,
};

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
        log::debug!("[Ext4Dentry::base_open]");
        match self.inode()?.itype() {
            InodeType::Dir => {
                let inode = self
                    .inode()?
                    .downcast_arc::<Ext4DirInode>()
                    .unwrap_or_else(|_| unreachable!());
                Ok(Ext4DirFile::new(self, inode))
            }
            InodeType::File => {
                let inode = self
                    .inode()?
                    .downcast_arc::<Ext4FileInode>()
                    .unwrap_or_else(|_| unreachable!());
                Ok(Ext4FileFile::new(self, inode))
            }
            InodeType::SymLink => {
                let inode = self
                    .inode()?
                    .downcast_arc::<Ext4LinkInode>()
                    .unwrap_or_else(|_| unreachable!());
                Ok(Ext4LinkFile::new(self, inode))
            }
            _ => todo!(),
        }
    }

    fn base_lookup(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {
        log::debug!("[Ext4Dentry::base_lookup] name: {name}");
        let sb = self.super_block();
        let sub_dentry = self.into_dyn().get_child(name).unwrap();
        let path = sub_dentry.path();
        if lwext4_check_inode_exist(&path, InodeTypes::EXT4_DE_DIR) {
            let new_file = LwExt4Dir::open(&path).map_err(SysError::from_i32)?;
            sub_dentry.set_inode(Ext4DirInode::new(sb, new_file))
        } else if lwext4_check_inode_exist(&path, InodeTypes::EXT4_DE_REG_FILE) {
            let new_file =
                LwExt4File::open(&path, OpenFlags::empty().bits()).map_err(SysError::from_i32)?;
            sub_dentry.set_inode(Ext4FileInode::new(sb, new_file))
        } else if lwext4_check_inode_exist(&path, InodeTypes::EXT4_DE_SYMLINK) {
            sub_dentry.set_inode(Ext4LinkInode::new(sb))
        }
        Ok(sub_dentry)
    }

    fn base_create(self: Arc<Self>, name: &str, mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        let types = match mode.to_type() {
            InodeType::Dir => InodeTypes::EXT4_DE_DIR,
            InodeType::File => InodeTypes::EXT4_DE_REG_FILE,
            _ => unimplemented!(),
        };

        let sb = self.super_block();
        let inode = self
            .inode()?
            .downcast_arc::<Ext4DirInode>()
            .unwrap_or_else(|_| unreachable!());
        let sub_dentry = self.into_dyn().get_child_or_create(name);
        let path = sub_dentry.path();
        log::debug!("[Ext4Dentry::base_create] path:{path}, mode:{mode:?}");
        let mut dir = inode.dir.lock();
        let new_inode: Arc<dyn Inode> = if types == InodeTypes::EXT4_DE_DIR {
            let new_dir = LwExt4Dir::create(&path).map_err(SysError::from_i32)?;
            Ext4DirInode::new(sb, new_dir)
        } else {
            let new_file = LwExt4File::open(
                &path,
                (OpenFlags::O_RDWR | OpenFlags::O_CREAT | OpenFlags::O_TRUNC).bits(),
            )
            .map_err(SysError::from_i32)?;
            Ext4FileInode::new(sb, new_file)
        };
        sub_dentry.set_inode(new_inode);
        Ok(sub_dentry)
    }

    fn base_unlink(self: Arc<Self>, name: &str) -> SysResult<()> {
        let sub_dentry = self.get_child(name).unwrap();
        let path = sub_dentry.path();
        if lwext4_check_inode_exist(&path, InodeTypes::EXT4_DE_DIR) {
            lwext4_rmdir(&path).map_err(SysError::from_i32)
        } else {
            lwext4_rmfile(&path).map_err(SysError::from_i32)
        }
    }

    fn base_new_child(self: Arc<Self>, name: &str) -> Arc<dyn Dentry> {
        Self::new(name, self.super_block(), Some(self))
    }

    fn base_rename_to(self: Arc<Self>, new: Arc<dyn Dentry>, flags: RenameFlags) -> SysResult<()> {
        // TODO: lwext4_rust does not support RENAME_EXCHANGE, it remove old path when
        // renaming
        let old_itype = self.inode()?.itype();
        if !new.is_negetive() {
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
                lwext4_mvdir(&self.path(), &new.path());
            }
            InodeType::File => {
                lwext4_mvfile(&self.path(), &new.path());
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
