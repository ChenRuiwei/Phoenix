use alloc::{boxed::Box, sync::Arc};

use log::debug;
use sync::mutex::SleepLock;
use systype::{AsyscallRet, GeneralRet, SyscallErr};

use crate::{
    fs::{
        fat32::SECTOR_SIZE,
        file::{FileMeta, FileMetaInner},
        inode::InodeMeta,
        File, Inode, InodeMode, Mutex, OpenFlags, FILE_SYSTEM_MANAGER,
    },
    processor::SumGuard,
    stack_trace,
};

pub struct MountsInode {
    metadata: InodeMeta,
}
impl MountsInode {
    pub fn new(parent: Arc<dyn Inode>, path: &str) -> Self {
        stack_trace!();
        Self {
            metadata: InodeMeta::new(Some(parent), path, InodeMode::FileREG, SECTOR_SIZE, None),
        }
    }
}

impl Inode for MountsInode {
    fn open(&self, this: Arc<dyn Inode>) -> GeneralRet<Arc<dyn File>> {
        stack_trace!();
        Ok(Arc::new(MountsFile {
            meta: FileMeta {
                inner: Mutex::new(FileMetaInner {
                    inode: Some(this),
                    mode: InodeMode::FileREG,
                    pos: 0,
                    dirent_index: 0,
                    file: None,
                }),
                prw_lock: SleepLock::new(()),
            },
        }))
    }
    fn metadata(&self) -> &InodeMeta {
        stack_trace!();
        &self.metadata
    }

    fn set_metadata(&mut self, meta: InodeMeta) {
        stack_trace!();
        self.metadata = meta;
    }

    fn load_children_from_disk(&self, _this: Arc<dyn Inode>) {
        stack_trace!();
        panic!("Unsupported operation")
    }

    fn delete_child(&self, _child_name: &str) {
        stack_trace!();
        panic!("Unsupported operation")
    }
    fn child_removeable(&self) -> GeneralRet<()> {
        stack_trace!();
        Err(SyscallErr::EPERM)
    }
}

pub struct MountsFile {
    meta: FileMeta,
}

impl File for MountsFile {
    fn read<'a>(&'a self, buf: &'a mut [u8], _flags: OpenFlags) -> AsyscallRet {
        stack_trace!();
        debug!("[MountsFile] read");
        Box::pin(async move {
            let _sum_guard = SumGuard::new();
            let info = FILE_SYSTEM_MANAGER.mounts_info();
            let len = info.len();
            let mut inner = self.metadata().inner.lock();
            if inner.pos == len {
                debug!("[MountFile] already read");
                Ok(0)
            } else {
                buf[..len].copy_from_slice(info.as_bytes());
                inner.pos = len;
                Ok(len)
            }
        })
    }

    fn write<'a>(&'a self, _buf: &'a [u8], _flags: OpenFlags) -> AsyscallRet {
        stack_trace!();
        debug!("[MountsFile] cannot write");
        Box::pin(async move { Err(SyscallErr::EACCES) })
    }

    fn metadata(&self) -> &FileMeta {
        stack_trace!();
        &self.meta
    }
}
