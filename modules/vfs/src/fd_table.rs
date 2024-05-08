use alloc::{sync::Arc, vec::Vec};

use systype::{SysError, SysResult};
use vfs_core::{File, OpenFlags};

use crate::devfs::{
    stdio::{StdInFile, StdOutFile},
    tty::{TtyFile, TTY},
};

pub type Fd = usize;

#[derive(Clone)]
pub struct FdTable {
    table: Vec<Option<Arc<dyn File>>>,
}

impl FdTable {
    pub fn new() -> Self {
        let mut vec: Vec<Option<Arc<dyn File>>> = Vec::new();
        // TODO: alloc stdio fd
        // vec.push(Some(StdInFile::new()));
        // vec.push(Some(StdOutFile::new()));
        // vec.push(Some(StdOutFile::new()));
        vec.push(Some(TTY.get().unwrap().clone()));
        vec.push(Some(TTY.get().unwrap().clone()));
        vec.push(Some(TTY.get().unwrap().clone()));

        Self { table: vec }
    }

    fn find_free_slot(&self) -> Option<usize> {
        (0..self.table.len()).find(|fd| self.table[*fd].is_none())
    }

    fn find_free_slot_and_create(&mut self, lower_bound: usize) -> usize {
        if lower_bound > self.table.len() {
            for _ in self.table.len()..lower_bound {
                self.table.push(None)
            }
            lower_bound
        } else {
            for idx in lower_bound..self.table.len() {
                if self.table[idx].is_none() {
                    return idx;
                }
            }
            self.table.push(None);
            self.table.len()
        }
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

    pub fn dup_with_bound(&mut self, old_fd: Fd, lower_bound: usize) -> SysResult<Fd> {
        let file = self.get(old_fd)?;
        let new_fd = self.find_free_slot_and_create(lower_bound);
        self.insert(new_fd, file);
        Ok(new_fd)
    }

    pub fn close_on_exec(&mut self) {
        for (_, slot) in self.table.iter_mut().enumerate() {
            if let Some(file) = slot {
                if file.flags().contains(OpenFlags::O_CLOEXEC) {
                    *slot = None;
                }
            }
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
