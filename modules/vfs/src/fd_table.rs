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
    table: Vec<Option<Arc<dyn File>>>,
}

impl FdTable {
    pub fn new() -> Self {
        let mut vec: Vec<Option<Arc<dyn File>>> = Vec::new();
        // TODO: alloc stdio fd
        vec.push(None);
        vec.push(None);
        vec.push(None);
        Self { table: vec }
    }

    fn find_free_slot(&self) -> Option<usize> {
        (0..self.table.len()).find(|fd| self.table[*fd].is_none())
    }

    // finds a fd and insert the file descriptor into the table
    pub fn alloc(&mut self, file: Arc<dyn File>) -> SysResult<usize> {
        if let Some(fd) = self.find_free_slot() {
            self.table[fd] = Some(file);
            Ok(fd)
        } else {
            self.table.push(Some(file));
            Ok(self.table.len() - 1)
        }
    }

    pub fn put(&mut self, fd: Fd, file: Arc<dyn File>) {
        assert!(fd < self.table.len());
        assert!(self.table[fd].is_none());
        self.table[fd] = Some(file);
    }

    pub fn get(&self, fd: Fd) -> Option<Arc<dyn File>> {
        if fd >= self.table.len() {
            None
        } else {
            self.table[fd].clone()
        }
    }

    /// Take the ownership of the given fd.
    pub fn take(&mut self, fd: Fd) -> Option<Arc<dyn File>> {
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
