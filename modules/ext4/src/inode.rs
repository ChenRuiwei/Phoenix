use alloc::sync::Arc;

use lwext4_rust::{bindings::{ext4_flink, O_RDONLY}, InodeTypes};
use systype::SysResult;
use vfs_core::{Inode, InodeMeta, InodeMode, Stat, SuperBlock};

use crate::{map_ext4_err, map_ext4_type, LwExt4File, Mutex, Shared};

pub struct Ext4Inode {
    meta: InodeMeta,
    pub file: Shared<LwExt4File>,
}

unsafe impl Send for Ext4Inode {}
unsafe impl Sync for Ext4Inode {}

impl Ext4Inode {
    pub fn new(super_block: Arc<dyn SuperBlock>, file: LwExt4File) -> Arc<Self> {
        let mut file = file;
        let path = file.get_path();
        let path = path.to_str().unwrap();
        let mut size = 0;
        if file.get_type() == InodeTypes::EXT4_DE_REG_FILE {
            file.file_open(path, O_RDONLY)
                .map_err(map_ext4_err)
                .unwrap();
            size = file.file_size();
            let _ = file.file_close();
        }
        let itype = map_ext4_type(file.get_type());

        let size: usize = size.try_into().unwrap();
        let inode = Arc::new(Self {
            meta: InodeMeta::new(InodeMode::from_type(itype), super_block.clone(), size),
            file: Arc::new(Mutex::new(file)),
        });
        super_block.push_inode(inode.clone());
        inode
    }
}

impl Inode for Ext4Inode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        let mode = self.meta.mode.bits();
        let len = inner.size;
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: mode,
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: len as u64,
            st_blksize: 512,
            __pad2: 0,
            st_blocks: (len / 512) as u64,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }
}
