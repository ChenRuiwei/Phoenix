use alloc::{rc::Weak, sync::Arc};

use systype::SysError;
use vfs_core::{Inode, InodeMeta, InodeMode, SuperBlock};

use super::file::FatFileInode;
use crate::{as_sys_err, dentry, new_shared, FatDir, Mutex, Shared};

pub struct FatDirInode {
    meta: InodeMeta,
    dir: Shared<FatDir>,
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
        let sb = dentry.meta().super_block.upgrade().unwrap();
        let name = dentry.name();
        match mode {
            InodeMode::Dir => {
                let new_dir = self.dir.lock().create_dir(&name).map_err(as_sys_err)?;
                let inode = FatDirInode::new(sb, new_dir);
                dentry.set_inode(inode);
                Ok(())
            }
            InodeMode::File => {
                let file = self.dir.lock().create_file(&name).map_err(as_sys_err)?;
                let inode = FatFileInode::new(sb, file);
                dentry.set_inode(inode);
                Ok(())
            }
            _ => Err(SysError::EIO),
        }
    }

    fn lookup(&self, dentry: Arc<dyn vfs_core::Dentry>) -> systype::SysResult<()> {
        let sb = self.meta().super_block.upgrade().unwrap();
        let name = dentry.name();
        let dir = self.dir.lock();
        if let Some(find) = dir.iter().find(|e| {
            let entry = e.as_ref().unwrap();
            let e_name = entry.file_name();
            name == e_name
        }) {
            let entry = find.map_err(as_sys_err)?;
            if entry.is_dir() {
                let new_dir = dir.open_dir(&name).map_err(as_sys_err)?;
                drop(dir);
                let inode = FatDirInode::new(sb, new_dir);
                dentry.set_inode(inode);
            } else {
                let file = dir.open_file(&name).map_err(as_sys_err)?;
                drop(dir);
                let inode = FatFileInode::new(sb, file);
                dentry.set_inode(inode);
            }
        } else {
            dentry.clear_inode();
        }
        Ok(())
    }

    fn get_attr(&self) -> systype::SysResult<vfs_core::Stat> {
        todo!()
    }
}
