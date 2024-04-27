use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};

use systype::{SysError, SysResult};
use vfs_core::File;

use crate::dev::stdio::{StdInFile, StdOutFile};

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
        vec.push(Some(StdInFile::new()));
        vec.push(Some(StdOutFile::new()));
        vec.push(Some(StdOutFile::new()));
        Self { table: vec }
    }

    fn find_free_slot(&self) -> Option<usize> {
        // FIXME: search from 0, howerver, it need fd table to have stdio file now
        (0..self.table.len()).find(|fd| self.table[*fd].is_none())
    }

    /// Find the minimium released fd, will alloc a fd if necessary, and insert
    /// the `file` into the table.
    pub fn alloc(&mut self, file: Arc<dyn File>) -> SysResult<Fd> {
        if let Some(fd) = self.find_free_slot() {
            self.table[fd] = Some(file);
            Ok(fd)
        } else {
            self.table.push(Some(file));
            Ok(self.table.len() - 1)
        }
    }

    pub fn get(&self, fd: Fd) -> SysResult<Arc<dyn File>> {
        if fd >= self.table.len() {
            Err(SysError::EBADF)
        } else {
            let file = self.table[fd].clone().ok_or(SysError::EBADF)?;
            Ok(file)
        }
    }

    pub fn remove(&mut self, fd: Fd) -> SysResult<()> {
        if fd >= self.table.len() {
            Err(SysError::EBADF)
        } else {
            self.table[fd] = None;
            Ok(())
        }
    }

    pub fn insert(&mut self, fd: Fd, file: Arc<dyn File>) -> SysResult<()> {
        if fd >= self.table.len() {
            for _ in self.table.len()..fd {
                self.table.push(None)
            }
            self.table.push(Some(file));
            Ok(())
        } else {
            self.table[fd] = Some(file);
            Ok(())
        }
    }

    pub fn dup(&mut self, old_fd: Fd) -> SysResult<Fd> {
        let file = self.get(old_fd)?;
        self.alloc(file)
    }

    pub fn dup3(&mut self, old_fd: Fd, new_fd: Fd) -> SysResult<Fd> {
        let file = self.get(old_fd)?;
        self.insert(new_fd, file)?;
        Ok(new_fd)
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
