use alloc::{
    sync::Arc,
    vec::{self, Vec},
};
use core::fmt;

use config::fs::MAX_FDS;
use systype::{RLimit, SysError, SysResult};
use vfs_core::{File, OpenFlags};

use crate::devfs::tty::TTY;

pub type Fd = usize;

#[derive(Clone)]
pub struct FdTable {
    table: Vec<Option<FdInfo>>,
    rlimit: RLimit,
}

bitflags::bitflags! {
    // Defined in <bits/fcntl-linux.h>.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FdFlags: u8 {
        const CLOEXEC = 1;
    }
}

impl From<OpenFlags> for FdFlags {
    fn from(value: OpenFlags) -> Self {
        if value.contains(OpenFlags::O_CLOEXEC) {
            FdFlags::CLOEXEC
        } else {
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

impl fmt::Debug for FdInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FdInfo")
            .field("flags", &self.flags)
            .field("file path", &self.file.dentry().path())
            .finish()
    }
}

impl FdInfo {
    pub fn new(file: Arc<dyn File>, flags: FdFlags) -> Self {
        Self { flags, file }
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
        let mut table: Vec<Option<FdInfo>> = Vec::with_capacity(MAX_FDS);

        let tty_file = TTY.get().unwrap().clone();
        let stdin = tty_file.clone();
        stdin.set_flags(OpenFlags::empty());
        let stdout = tty_file.clone();
        stdout.set_flags(OpenFlags::O_WRONLY);
        let stderr = tty_file.clone();
        stderr.set_flags(OpenFlags::O_WRONLY);

        table.push(Some(FdInfo::new(stdin, FdFlags::empty())));
        table.push(Some(FdInfo::new(stdout, FdFlags::empty())));
        table.push(Some(FdInfo::new(stderr, FdFlags::empty())));

        Self {
            table,
            rlimit: RLimit {
                rlim_cur: MAX_FDS,
                rlim_max: MAX_FDS,
            },
        }
    }

    fn get_free_slot(&mut self) -> Option<usize> {
        let inner_slot = self
            .table
            .iter()
            .enumerate()
            .find(|(i, e)| e.is_none())
            .map(|(i, _)| i);
        if inner_slot.is_some() {
            return inner_slot;
        } else if inner_slot.is_none() && self.table.len() < self.rlimit.rlim_max {
            self.table.push(None);
            return Some(self.table.len() - 1);
        } else {
            return None;
        }
    }

    fn get_free_slot_from(&mut self, start: usize) -> Option<usize> {
        let inner_slot = self
            .table
            .iter()
            .enumerate()
            .skip(start)
            .find(|(i, e)| e.is_none())
            .map(|(i, _)| i);
        if inner_slot.is_some() {
            return inner_slot;
        } else if inner_slot.is_none() && start < self.rlimit.rlim_max {
            // if table len not enough, push enough empty slots
            for _ in self.table.len()..start {
                self.table.push(None);
            }
            // inner_slot is none means we need to add more one slot
            self.table.push(None);
            return Some(self.table.len() - 1);
        } else {
            return None;
        }
    }

    fn extend_to(&mut self, len: usize) -> SysResult<()> {
        if len > self.rlimit.rlim_max {
            return Err(SysError::EBADF);
        } else if self.table.len() >= len {
            return Ok(());
        } else {
            for _ in self.table.len()..len {
                self.table.push(None)
            }
            Ok(())
        }
    }

    /// Find the minimium released fd, will alloc a fd if necessary, and insert
    /// the `file` into the table.
    pub fn alloc(&mut self, file: Arc<dyn File>, flags: OpenFlags) -> SysResult<Fd> {
        let fd_info = FdInfo::new(file, flags.into());
        if let Some(fd) = self.get_free_slot() {
            self.table[fd] = Some(fd_info);
            Ok(fd)
        } else {
            Err(SysError::EMFILE)
        }
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

    pub fn get_file(&self, fd: Fd) -> SysResult<Arc<dyn File>> {
        Ok(self.get(fd)?.file())
    }

    pub fn remove(&mut self, fd: Fd) -> SysResult<()> {
        if fd >= self.table.len() {
            Err(SysError::EBADF)
        } else if self.table[fd].is_none() {
            Err(SysError::EBADF)
        } else {
            self.table[fd] = None;
            Ok(())
        }
    }

    pub fn put(&mut self, fd: Fd, fd_info: FdInfo) -> SysResult<()> {
        self.extend_to((fd.checked_add(1).ok_or(SysError::EBADF)?))?;
        self.table[fd] = Some(fd_info);
        Ok(())
    }

    /// Dup with no file descriptor flags.
    pub fn dup(&mut self, old_fd: Fd) -> SysResult<Fd> {
        let file = self.get_file(old_fd)?;
        self.alloc(file, OpenFlags::empty())
    }

    pub fn dup3(&mut self, old_fd: Fd, new_fd: Fd, flags: OpenFlags) -> SysResult<Fd> {
        let file = self.get_file(old_fd)?;
        let fd_info = FdInfo::new(file, flags.into());
        self.put(new_fd, fd_info)?;
        Ok(new_fd)
    }

    pub fn dup_with_bound(
        &mut self,
        old_fd: Fd,
        lower_bound: usize,
        flags: OpenFlags,
    ) -> SysResult<Fd> {
        let file = self.get_file(old_fd)?;
        let new_fd = self
            .get_free_slot_from(lower_bound)
            .ok_or_else(|| SysError::EMFILE)?;
        let fd_info = FdInfo::new(file, flags.into());
        self.put(new_fd, fd_info)?;
        debug_assert!(new_fd >= lower_bound);
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

    pub fn rlimit(&self) -> RLimit {
        self.rlimit
    }

    pub fn set_rlimit(&mut self, rlimit: RLimit) {
        self.rlimit = rlimit;
        if rlimit.rlim_max <= self.table.len() {
            self.table.truncate(self.rlimit.rlim_max)
        }
    }
}
