use alloc::{
    boxed::Box,
    ffi::CString,
    string::String,
    sync::{self, Arc},
    vec::Vec,
};
use core::iter::zip;

use async_trait::async_trait;
use lwext4_rust::bindings::{O_RDONLY, O_RDWR, SEEK_SET};
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::{DirEntry, File, FileMeta, Inode, InodeType};

use crate::{dentry::Ext4Dentry, inode::Ext4Inode, LwExt4File, Shared};

pub struct Ext4File {
    meta: FileMeta,
    file: Shared<LwExt4File>,
}

unsafe impl Send for Ext4File {}
unsafe impl Sync for Ext4File {}

impl Ext4File {
    pub fn new(dentry: Arc<Ext4Dentry>, inode: Arc<Ext4Inode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone(), inode.clone()),
            file: inode.file.clone(),
        })
    }
}

#[async_trait]
impl File for Ext4File {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        log::error!(
            "offset {offset} in blk id {}",
            self.inode().get_blk_idx(offset as u64)?
        );
        match self.itype() {
            InodeType::File | InodeType::SymLink => {
                let mut file = self.file.lock();
                let path = file.get_path();
                let path = path.to_str().unwrap();
                // file.file_open(path, O_RDONLY).map_err(SysError::from_i32)?;
                file.file_seek(offset as i64, SEEK_SET)
                    .map_err(SysError::from_i32)?;
                let r = file.file_read(buf).map_err(SysError::from_i32);
                // let _ = file.file_close();
                r
            }
            InodeType::Dir => Err(SysError::EISDIR),
            _ => unreachable!(),
        }
    }

    async fn base_write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        match self.itype() {
            InodeType::File => {
                let mut file = self.file.lock();
                let path = file.get_path();
                let path = path.to_str().unwrap();
                // file.file_open(path, O_RDWR).map_err(SysError::from_i32)?;
                file.file_seek(offset as i64, SEEK_SET)
                    .map_err(SysError::from_i32)?;
                let r = file.file_write(buf).map_err(SysError::from_i32);
                // let _ = file.file_close();
                r
            }
            InodeType::Dir => Err(SysError::EISDIR),
            _ => unreachable!(),
        }
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        todo!()
    }

    /// Load all dentry and inodes in a directory. Will not advance dir offset.
    fn base_load_dir(&self) -> SysResult<()> {
        let file = self.file.lock();
        let iters = file.lwext4_dir_entries().unwrap();

        let path = self.dentry().path();
        for (name, file_type) in zip(iters.0, iters.1).skip(3) {
            let name = CString::from_vec_with_nul(name).map_err(|_| SysError::EINVAL)?;
            let name = name.to_str().unwrap();
            let sub_dentry = self.dentry().get_child_or_create(name);
            let ext4_file = LwExt4File::new(&(sub_dentry.path()), file_type);
            let new_inode: Arc<dyn Inode> = Ext4Inode::new(self.super_block(), ext4_file);
            sub_dentry.set_inode(new_inode);
        }
        Ok(())
    }
}
