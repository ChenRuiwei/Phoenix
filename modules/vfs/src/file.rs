use alloc::sync::{Arc, Weak};

use systype::{SysError, SysResult};

use crate::Inode;

pub struct FileMeta {
    pub inode: Option<Arc<dyn Inode>>,
    pub file: Option<Weak<dyn File>>,
}

pub trait File: Send + Sync {
    fn read(&self, _offset: usize, _buf: &mut [u8]) -> SysResult<usize> {
        Err(SysError::ENOSYS)
    }

    fn write(&self, _offset: usize, _buf: &[u8]) -> SysResult<usize> {
        Err(SysError::ENOSYS)
    }
}
