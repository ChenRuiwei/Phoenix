use alloc::{
    boxed::Box,
    ffi::CString,
    string::String,
    sync::{self, Arc},
    vec::Vec,
};
use core::{cmp, iter::zip};

use async_trait::async_trait;
use lwext4_rust::{
    bindings::{O_RDONLY, O_RDWR, SEEK_SET},
    lwext4_readlink, InodeTypes,
};
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::{DirEntry, File, FileMeta, Inode, InodeType, OpenFlags};

use crate::{
    dentry::Ext4Dentry, inode::Ext4FileInode, map_ext4_type, Ext4DirInode, Ext4LinkInode,
    LwExt4Dir, LwExt4File, Shared,
};

pub struct Ext4DirFile {
    meta: FileMeta,
    dir: Shared<LwExt4Dir>,
}

unsafe impl Send for Ext4DirFile {}
unsafe impl Sync for Ext4DirFile {}

impl Ext4DirFile {
    pub fn new(dentry: Arc<Ext4Dentry>, inode: Arc<Ext4DirInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone(), inode.clone()),
            dir: inode.dir.clone(),
        })
    }
}

#[async_trait]
impl File for Ext4DirFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        Err(SysError::EISDIR)
    }

    async fn base_write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        Err(SysError::EISDIR)
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        todo!()
    }

    /// Load all dentry and inodes in a directory. Will not advance dir offset.
    fn base_load_dir(&self) -> SysResult<()> {
        let mut dir = self.dir.lock();
        let iters = dir.lwext4_dir_entries(&self.dentry().path()).unwrap();

        // skip "." and ".."
        dir.next();
        dir.next();

        while let Some(dirent) = dir.next() {
            let name = CString::new(dirent.name).map_err(|_| SysError::EINVAL)?;
            let name = name.to_str().unwrap();
            let sub_dentry = self.dentry().get_child_or_create(name);
            let new_inode: Arc<dyn Inode> = if InodeTypes::from(dirent.type_ as usize)
                == InodeTypes::EXT4_DE_REG_FILE
            {
                let ext4_file = LwExt4File::open(&(sub_dentry.path()), OpenFlags::O_RDWR.bits())
                    .map_err(SysError::from_i32)?;
                Ext4FileInode::new(self.super_block(), ext4_file).clone()
            } else if InodeTypes::from(dirent.type_ as usize) == InodeTypes::EXT4_DE_DIR {
                let ext4_dir = LwExt4Dir::open(&(sub_dentry.path())).map_err(SysError::from_i32)?;
                Ext4DirInode::new(self.super_block(), ext4_dir).clone()
            } else {
                Ext4LinkInode::new(self.super_block()).clone()
            };
            sub_dentry.set_inode(new_inode);
        }

        Ok(())
    }
}
