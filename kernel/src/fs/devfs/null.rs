use alloc::{boxed::Box, sync::Arc};

use log::debug;
use sync::mutex::SleepLock;
use systype::{AsyscallRet, GeneralRet, SyscallErr, SyscallRet};

use crate::{
    fs::{
        file::{FileMeta, FileMetaInner},
        inode::InodeMeta,
        File, Inode, Mutex, OpenFlags,
    },
    stack_trace,
};

pub struct NullInode {
    metadata: InodeMeta,
    // dev_fs: SyncUnsafeCell<Option<Arc<DevFs>>>,
}

impl NullInode {
    pub fn new(parent: Arc<dyn Inode>, path: &str) -> Self {
        stack_trace!();
        let metadata = InodeMeta::new(Some(parent), path, crate::fs::InodeMode::FileCHR, 0, None);
        Self { metadata }
    }
}

impl Inode for NullInode {
    fn open(&self, this: Arc<dyn Inode>) -> GeneralRet<Arc<dyn File>> {
        stack_trace!();
        Ok(Arc::new(NullFile {
            meta: FileMeta {
                inner: Mutex::new(FileMetaInner {
                    inode: Some(this),
                    mode: self.metadata.mode,
                    pos: 0,
                    dirent_index: 0,
                    file: None,
                }),
                prw_lock: SleepLock::new(()),
                // path: self.metadata().path.clone(),
            },
        }))
    }
    fn set_metadata(&mut self, meta: InodeMeta) {
        stack_trace!();
        self.metadata = meta;
    }
    fn metadata(&self) -> &InodeMeta {
        &self.metadata
    }
    fn load_children_from_disk(&self, _this: Arc<dyn Inode>) {
        panic!("Unsupported operation")
    }
    fn delete_child(&self, _child_name: &str) {
        panic!("Unsupported operation delete")
    }
    fn child_removeable(&self) -> GeneralRet<()> {
        Err(SyscallErr::EPERM)
    }
}

pub struct NullFile {
    meta: FileMeta,
}

// #[async_trait]
impl File for NullFile {
    fn metadata(&self) -> &FileMeta {
        &self.meta
    }
    fn read<'a>(&'a self, _buf: &'a mut [u8], _flags: OpenFlags) -> AsyscallRet {
        debug!("[read] /dev/null");
        Box::pin(async move { Ok(0) })
    }
    fn write<'a>(&'a self, buf: &'a [u8], _flags: OpenFlags) -> AsyscallRet {
        debug!("[write] /dev/null");
        Box::pin(async move { Ok(buf.len()) })
    }
    fn sync_read(&self, _buf: &mut [u8]) -> SyscallRet {
        debug!("[sync_read] /dev/null");
        Ok(0)
    }
    fn sync_write(&self, buf: &[u8]) -> SyscallRet {
        debug!("[sync_write] /dev/null");
        Ok(buf.len())
    }
}