use alloc::{sync::Arc, vec::Vec};

use systype::{SysError, SysResult};
use vfs_core::{File, OpenFlags};

use crate::devfs::{stdio, tty::TTY};

pub type Fd = usize;

const MAX_FD_NUM_DEFAULT: usize = 1024;

#[derive(Clone)]
pub struct FdTable {
    table: Vec<Option<FdInfo>>,
    // TODO: add code for making sure tha FdTable length is less than `limit`
    limit: usize,
}

bitflags::bitflags! {
    // Defined in <bits/fcntl-linux.h>.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FdFlags: isize {
        const CLOEXEC = 1;
    }
}

impl From<OpenFlags> for FdFlags {
    fn from(value: OpenFlags) -> Self {
        if value.contains(OpenFlags::O_CLOEXEC) {
            FdFlags::CLOEXEC
        } else {
            log::warn!("[FdFlags::from] unsupported flag");
            FdFlags::empty()
        }
    }
}

#[derive(Clone)]
pub struct FdInfo {
    /// File descriptor flags, currently, only one such flag is defined:
    /// FD_CLOEXEC.
    flags: FdFlags,
    file: Arc<dyn File>,
}

impl FdInfo {
    pub fn new(file: Arc<dyn File>) -> Self {
        Self {
            flags: FdFlags::empty(),
            file,
        }
    }

    pub fn file(&self) -> Arc<dyn File> {
        self.file.clone()
    }

    pub fn flags(&self) -> FdFlags {
        self.flags
    }

    pub fn set_flags(&mut self, flags: FdFlags) {
        self.flags = flags
    }

    pub fn set_close_on_exec(&mut self) {
        self.flags = FdFlags::CLOEXEC;
    }
}

impl FdTable {
    pub fn new() -> Self {
        let mut vec: Vec<Option<FdInfo>> = Vec::new();
        let tty_file = TTY.get().unwrap().clone();
        let stdin = tty_file.clone();
        stdin.set_flags(OpenFlags::empty());
        let stdout = tty_file.clone();
        stdout.set_flags(OpenFlags::O_WRONLY);
        let stderr = tty_file.clone();
        stderr.set_flags(OpenFlags::O_WRONLY);

        vec.push(Some(FdInfo::new(stdin)));
        vec.push(Some(FdInfo::new(stdout)));
        vec.push(Some(FdInfo::new(stderr)));
        Self {
            table: vec,
            limit: MAX_FD_NUM_DEFAULT,
        }
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
        let fd_info = FdInfo::new(file);
        if let Some(fd) = self.find_free_slot() {
            self.table[fd] = Some(fd_info);
            Ok(fd)
        } else {
            self.table.push(Some(fd_info));
            Ok(self.table.len() - 1)
        }
    }

    pub fn get_file(&self, fd: Fd) -> SysResult<Arc<dyn File>> {
        Ok(self.get(fd)?.file())
    }

    pub fn get(&self, fd: Fd) -> SysResult<&FdInfo> {
        if fd >= self.table.len() {
            Err(SysError::EBADF)
        } else {
            self.table[fd].as_ref().ok_or(SysError::EBADF)
        }
    }

    pub fn get_mut(&mut self, fd: Fd) -> SysResult<&mut FdInfo> {
        if fd >= self.table.len() {
            Err(SysError::EBADF)
        } else {
            self.table[fd].as_mut().ok_or(SysError::EBADF)
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
        let fd_info = FdInfo::new(file);
        if fd >= self.table.len() {
            for _ in self.table.len()..fd {
                self.table.push(None)
            }
            self.table.push(Some(fd_info));
            Ok(())
        } else {
            self.table[fd] = Some(fd_info);
            Ok(())
        }
    }

    /// Dup with no file descriptor flags.
    pub fn dup(&mut self, old_fd: Fd) -> SysResult<Fd> {
        let file = self.get_file(old_fd)?;
        self.alloc(file)
    }

    pub fn dup3(&mut self, old_fd: Fd, new_fd: Fd, flags: OpenFlags) -> SysResult<Fd> {
        let file = self.get_file(old_fd)?;
        self.insert(new_fd, file)?;
        if flags.contains(OpenFlags::O_CLOEXEC) {
            self.table[new_fd].as_mut().unwrap().set_close_on_exec();
        }
        Ok(new_fd)
    }

    pub fn dup_with_bound(
        &mut self,
        old_fd: Fd,
        lower_bound: usize,
        flags: OpenFlags,
    ) -> SysResult<Fd> {
        let file = self.get_file(old_fd)?;
        let new_fd = self.find_free_slot_and_create(lower_bound);
        self.insert(new_fd, file)?;
        if flags.contains(OpenFlags::O_CLOEXEC) {
            self.table[new_fd].as_mut().unwrap().set_close_on_exec();
        }
        Ok(new_fd)
    }

    pub fn do_close_on_exec(&mut self) {
        for slot in self.table.iter_mut() {
            if let Some(fd_info) = slot {
                if fd_info.flags().contains(FdFlags::CLOEXEC) {
                    *slot = None;
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
    }
}
