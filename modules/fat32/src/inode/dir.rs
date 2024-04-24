use alloc::{rc::Weak, sync::Arc};

use systype::SysError;
use vfs_core::{Dentry, Inode, InodeMeta, InodeMode, SuperBlock};

use super::file::FatFileInode;
use crate::{
    as_sys_err,
    dentry::{self, FatDentry},
    new_shared, FatDir, Mutex, Shared,
};

pub struct FatDirInode {
    meta: InodeMeta,
    pub dir: Shared<FatDir>,
}

impl FatDirInode {
    pub fn new(super_block: Arc<dyn SuperBlock>, dir: FatDir) -> Arc<Self> {
        // TODO: Dir size is zero?
        let inode = Arc::new(Self {
            meta: InodeMeta::new(InodeMode::Dir, super_block.clone(), 0),
            dir: Arc::new(Mutex::new(dir)),
        });
        super_block.push_inode(inode.clone());
        inode
    }
}

impl Inode for FatDirInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn create(&self, dentry: Arc<dyn vfs_core::Dentry>, mode: InodeMode) -> systype::SysResult<()> {
        let sb = dentry.super_block();
        let name = dentry.name();
        match mode {
            InodeMode::Dir => {
                let new_dir = self.dir.lock().create_dir(&name).map_err(as_sys_err)?;
                let inode = FatDirInode::new(sb, new_dir);
                dentry.set_inode(inode);
                Ok(())
            }
            InodeMode::File => {
                let new_file = self.dir.lock().create_file(&name).map_err(as_sys_err)?;
                let inode = FatFileInode::new(sb, new_file);
                dentry.set_inode(inode);
                Ok(())
            }
            _ => Err(SysError::EIO),
        }
    }

    fn get_attr(&self) -> systype::SysResult<vfs_core::Stat> {
        todo!()
    }
}
