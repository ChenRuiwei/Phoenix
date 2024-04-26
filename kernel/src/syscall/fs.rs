use alloc::{string::ToString, sync::Arc, vec::Vec};

use systype::{SysError, SysResult, SyscallResult};
use vfs::sys_root_dentry;
use vfs_core::{is_relative_path, Dentry, OpenFlags, Path, AT_FDCWD};

use crate::{
    mm::{UserReadPtr, UserWritePtr},
    processor::hart::current_task,
};

// "brk",
// "chdir",
// "clone",
// "close",
// "dup",
// "dup2",
// "execve",
// "exit",
// "fork",
// "fstat",
// "getcwd",
// "getdents",
// "getpid",
// "getppid",
// "gettimeofday",
// "mkdir_",
// "mmap",
// "mnt",
// "mount",
// "munmap",
// "open",
// "openat",
// "pipe",
// "read",
// "run-all.sh",
// "sleep",
// "test_echo",
// "text.txt",
// "times",
// "umount",
// "uname",
// "unlink",
// "wait",
// "waitpid",
// "write",
// "yield",

// TODO:
pub async fn sys_write(fd: usize, buf: UserReadPtr<u8>, len: usize) -> SyscallResult {
    let task = current_task();
    let buf = buf.read_array(task, len)?;
    // TODO: now do not support char device
    if fd == 1 {
        for &b in buf.iter() {
            print!("{}", b as char);
        }
        return Ok(buf.len());
    } else {
        // get file and write
    }
    let file = task.with_fd_table(|table| table.get(fd).ok_or(SysError::EBADF))?;
    let ret = file.write(file.pos(), &buf)?;
    Ok(ret)
}

pub async fn sys_read(fd: usize, buf: UserWritePtr<u8>, len: usize) -> SyscallResult {
    let task = current_task();
    let file = task.with_fd_table(|table| table.get(fd).ok_or(SysError::EBADF))?;
    if file.inode().node_type().is_dir() {
        return Err(SysError::EISDIR);
    }
    let mut buf = buf.into_mut_slice(task, len)?;
    let ret = file.read(file.pos(), &mut buf)?;
    // log::debug!("{:?}", buf);
    Ok(ret)
}

// TODO:
pub fn sys_openat(dirfd: isize, pathname: UserReadPtr<u8>, flags: i32, mode: u32) -> SyscallResult {
    let task = current_task();
    let flags = OpenFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    let pathname = pathname.read_cstr(task)?;
    // FIXME: with flags O_CREAT
    let dentry = at_helper(dirfd, &pathname)?;
    let file = dentry.open()?;
    task.with_mut_fd_table(|table| table.alloc(file))
}

pub fn sys_close(fd: usize) -> SyscallResult {
    let task = current_task();
    task.with_mut_fd_table(|table| table.remove(fd))?;
    Ok(0)
}

/// The dirfd argument is used in conjunction with the pathname argument as
/// follows:
/// + If the pathname given in pathname is absolute, then dirfd is ignored.
/// + If the pathname given in pathname is relative and dirfd is the special
///   value AT_FDCWD, then pathname is interpreted relative to the current
///   working directory of the calling process (like open()).
/// + If the pathname given in pathname is relative, then it is interpreted
///   relative to the directory referred to by the file descriptor dirfd (rather
///   than relative to the current working directory of the calling process, as
///   is done by open() for a relative pathname).  In this case, dirfd must be a
///   directory that was opened for reading (O_RDONLY) or using the O_PATH flag.
fn at_helper(fd: isize, path: &str) -> SysResult<Arc<dyn Dentry>> {
    log::info!("[at_helper] fd: {},path:{}", fd, path);
    let task = current_task();
    let path = if is_relative_path(path) {
        if fd as i32 == AT_FDCWD {
            Path::new(sys_root_dentry(), task.cwd(), path)
        } else {
            let fd = fd as usize;
            let file = task.with_fd_table(|table| table.get(fd).ok_or(SysError::EBADF))?;
            Path::new(sys_root_dentry(), file.dentry(), path)
        }
    } else {
        Path::new(sys_root_dentry(), sys_root_dentry(), "")
    };
    path.walk()
}
