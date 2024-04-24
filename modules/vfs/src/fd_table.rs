use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};

use systype::SysResult;
use vfs_core::File;

pub type Fd = usize;

#[derive(Clone)]
pub struct FileRef {
    pub file: Arc<dyn File>,
}

impl FileRef {
    pub fn new(file: Arc<dyn File>) -> Self {
        Self { file }
    }
}

pub struct FdTable {
    pub table: Vec<Option<FileRef>>,
}

impl FdTable {
    pub fn new() -> Self {
        Self { table: Vec::new() }
    }

    fn find_free_slot(&self) -> Option<usize> {
        (0..self.table.len()).find(|fd| self.table[*fd].is_none())
    }

    // alloc finds a fd and insert the file descriptor into the table
    pub fn alloc(&mut self) -> SysResult<usize> {
        if let Some(fd) = self.find_free_slot() {
            Ok(fd)
        } else {
            self.table.push(None);
            Ok(self.table.len() - 1)
        }
    }

    pub fn put(&mut self, fd: Fd, file: FileRef) {
        assert!(fd < self.table.len());
        assert!(self.table[fd].is_none());
        self.table[fd] = Some(file);
    }

    /// Get the ownership of the given fd by clone
    pub fn get(&self, fd: Fd) -> Option<FileRef> {
        if fd >= self.table.len() {
            None
        } else {
            self.table[fd].clone()
        }
    }

    /// Get the ownership of the given fd by clone
    pub fn get_mut(&mut self, fd: Fd) -> Option<&mut FileRef> {
        if fd >= self.table.len() {
            None
        } else {
            self.table[fd].as_mut()
        }
    }

    /// Take the ownership of the given fd
    pub fn take(&mut self, fd: Fd) -> Option<FileRef> {
        if fd >= self.table.len() {
            None
        } else {
            self.table[fd].take()
        }
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }
}
