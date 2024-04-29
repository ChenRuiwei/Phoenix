use alloc::sync::Arc;

use driver::print;
use vfs_core::{File, FileMeta, Inode, InodeMeta, InodeMode};

// TODO: This file has a lot to do

pub struct StdOutInode {
    meta: InodeMeta,
}

pub struct StdOutFile {
    meta: FileMeta,
}

impl StdOutFile {
    pub fn new() -> Arc<Self> {
        let inode = Arc::new(StdOutInode {
            meta: InodeMeta::new(InodeMode::CHAR, Arc::<usize>::new_zeroed(), 0),
        });
        Arc::new(Self {
            meta: FileMeta::new(Arc::<usize>::new_zeroed(), inode),
        })
    }
}

impl Inode for StdOutInode {
    fn meta(&self) -> &vfs_core::InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> systype::SysResult<vfs_core::Stat> {
        todo!()
    }
}

impl File for StdOutFile {
    fn meta(&self) -> &vfs_core::FileMeta {
        &self.meta
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> systype::SysResult<usize> {
        todo!()
    }

    fn write(&self, offset: usize, buf: &[u8]) -> systype::SysResult<usize> {
        if let Ok(data) = core::str::from_utf8(buf) {
            print!("{}", data);
        } else {
            (0..buf.len()).for_each(|i| {
                log::warn!("User stderr (non-utf8): {} ", buf[i]);
            });
        }
        Ok(buf.len())
    }

    fn read_dir(&self) -> systype::SysResult<Option<vfs_core::DirEntry>> {
        todo!()
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }
}

pub struct StdInInode {
    meta: InodeMeta,
}

pub struct StdInFile {
    meta: FileMeta,
}

impl StdInFile {
    pub fn new() -> Arc<Self> {
        let inode = Arc::new(StdInInode {
            meta: InodeMeta::new(InodeMode::CHAR, Arc::<usize>::new_zeroed(), 0),
        });
        Arc::new(Self {
            meta: FileMeta::new(Arc::<usize>::new_zeroed(), inode.clone()),
        })
    }
}

impl Inode for StdInInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> systype::SysResult<vfs_core::Stat> {
        todo!()
    }
}

impl File for StdInFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn read(&self, offset: usize, buf: &mut [u8]) -> systype::SysResult<usize> {
        todo!()
    }

    fn write(&self, offset: usize, buf: &[u8]) -> systype::SysResult<usize> {
        todo!()
    }

    fn read_dir(&self) -> systype::SysResult<Option<vfs_core::DirEntry>> {
        todo!()
    }

    fn flush(&self) -> systype::SysResult<usize> {
        todo!()
    }
}
