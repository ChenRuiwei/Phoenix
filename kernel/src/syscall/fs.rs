use alloc::{ffi::CString, sync::Arc};

use systype::{SysError, SysResult, SyscallResult};
use vfs::sys_root_dentry;
use vfs_core::{is_absolute_path, Dentry, InodeMode, OpenFlags, Path, AT_FDCWD};

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
    // if fd == 1 {
    //     for &b in buf.iter() {
    //         print!("{}", b as char);
    //     }
    //     Ok(buf.len())
    // }

    // get file and write
    let file = task.with_fd_table(|table| table.get(fd))?;
    let ret = file.write(file.pos(), &buf)?;
    Ok(ret)
}

/// read() attempts to read up to count bytes from file descriptor fd into the
/// buffer starting at buf.
///
/// On success, the number of bytes read is returned (zero indicates end of
/// file), and the file position is advanced by this number.
pub async fn sys_read(fd: usize, buf: UserWritePtr<u8>, len: usize) -> SyscallResult {
    let task = current_task();
    let file = task.with_fd_table(|table| table.get(fd))?;
    if file.itype().is_dir() {
        return Err(SysError::EISDIR);
    }
    let mut buf = buf.into_mut_slice(task, len)?;
    let ret = file.read(file.pos(), &mut buf)?;
    Ok(ret)
}

/// The open() system call opens the file specified by pathname. If the
/// specified file does not exist, it may optionally (if O_CREAT is specified in
/// flags) be created by open().
///
/// The return value of open() is a file descriptor, a small, nonnegative
/// integer that is an index to an entry in the process's table of open file
/// descriptors. The file descriptor is used in subsequent system calls
/// (read(2), write(2), lseek(2), fcntl(2), etc.) to refer to the open file. The
/// file descriptor returned by a successful call will be the lowest-numbered
/// file descriptor not currently open for the process.
///
/// The mode argument specifies the file mode bits to be applied when a new file
/// is created. If neither O_CREAT nor O_TMPFILE is specified in flags, then
/// mode is ignored (and can thus be specified as 0, or simply omitted). The
/// mode argument must be supplied if O_CREAT or O_TMPFILE is specified in
/// flags; if it is not supplied, some arbitrary bytes from the stack will be
/// applied as the file mode.
// TODO:
pub fn sys_openat(dirfd: isize, pathname: UserReadPtr<u8>, flags: i32, mode: u32) -> SyscallResult {
    let task = current_task();
    let flags = OpenFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    let mode = InodeMode::from_bits_truncate(mode);
    let pathname = pathname.read_cstr(task)?;
    log::debug!("[sys_openat] {flags:?}, {mode:?}");
    let dentry = at_helper(dirfd, &pathname, flags, mode)?;
    let file = dentry.open()?;
    task.with_mut_fd_table(|table| table.alloc(file))
}

/// close() closes a file descriptor, so that it no longer refers to any file
/// and may be reused. Any record locks (see fcntl(2)) held on the file it was
/// associated with, and owned by the process, are removed regardless of the
/// file descriptor that was used to obtain the lock. This has some unfortunate
/// consequences and one should be extra careful when using advisory record
/// locking. See fcntl(2) for discussion of the risks and consequences as well
/// as for the (probably preferred) open file description locks.
///
/// close() returns zero on success.  On error, -1 is returned, and errno is set
/// to indicate the error.
pub fn sys_close(fd: usize) -> SyscallResult {
    let task = current_task();
    task.with_mut_fd_table(|table| table.remove(fd))?;
    Ok(0)
}

/// mkdirat() attempts to create a directory named pathname.
///
/// mkdir() and mkdirat() return zero on success.  On error, -1 is returned and
/// errno is set to indicate the error.
pub fn sys_mkdirat(dirfd: isize, pathname: UserReadPtr<u8>, mode: u32) -> SyscallResult {
    let task = current_task();
    let mode = InodeMode::from_bits_truncate(mode);
    let pathname = pathname.read_cstr(task)?;
    log::debug!("[sys_mkdirat] {mode:?}");
    let dentry = at_helper(dirfd, &pathname, OpenFlags::empty(), mode)?;
    if !dentry.is_negetive() {
        return Err(SysError::EEXIST);
    }
    dentry
        .parent()
        .unwrap()
        .create(dentry.name(), mode.union(InodeMode::DIR))?;
    Ok(0)
}

/// These functions return a null-terminated string containing an absolute
/// pathname that is the current working directory of the calling process. The
/// pathname is returned as the function result and via the argument buf, if
/// present.
///
/// The getcwd() function copies an absolute pathname of the current working
/// directory to the array pointed to by buf, which is of length size.
///
/// If the length of the absolute pathname of the current working directory,
/// including the terminating null byte, exceeds size bytes, NULL is returned,
/// and errno is set to ERANGE; an application should check for this error, and
/// allocate a larger buffer if necessary.
///
/// On success, these functions return a pointer to a string containing the
/// pathname of the current working directory. In the case of getcwd() and
/// getwd() this is the same value as buf.
///
/// On failure, these functions return NULL, and errno is set to indicate the
/// error. The contents of the array pointed to by buf are undefined on error.
pub fn sys_getcwd(buf: UserWritePtr<u8>, size: usize) -> SyscallResult {
    if size == 0 && buf.not_null() {
        return Err(SysError::EINVAL);
    }
    if buf.is_null() {
        return Err(SysError::EINVAL);
    }
    let task = current_task();
    let abs_path = task.cwd().path();
    let c_path_len = abs_path.len() + 1;
    if c_path_len > size {
        return Err(SysError::ERANGE);
    }
    let length = core::cmp::min(c_path_len, size);
    let abs_path = CString::new(abs_path).expect("can not have null byte in c string");
    let ret = buf.as_usize();
    buf.into_mut_slice(task, length)?
        .copy_from_slice(&abs_path.into_bytes_with_nul());
    Ok(ret)
}

/// chdir() changes the current working directory of the calling process to the
/// directory specified in path.
///
/// On success, zero is returned.  On error, -1 is returned, and errno is set to
/// indicate the error.
pub fn sys_chdir(path: UserReadPtr<u8>) -> SyscallResult {
    let task = current_task();
    let path = path.read_cstr(task)?;
    log::debug!("[sys_chdir] path {path}");
    let dentry = resolve_path(&path)?;
    if !dentry.inode()?.itype().is_dir() {
        return Err(SysError::ENOTDIR);
    }
    task.set_cwd(dentry);
    Ok(0)
}

/// The dup() system call allocates a new file descriptor that refers to the
/// same open file description as the descriptor oldfd. (For an explanation of
/// open file descriptions, see open(2).) The new file descriptor number is
/// guaranteed to be the lowest-numbered file descriptor that was unused in the
/// calling process.
///
/// After a successful return, the old and new file descriptors may be used
/// interchangeably. Since the two file descriptors refer to the same open file
/// description, they share file offset and file status flags; for example, if
/// the file offset is modified by using lseek(2) on one of the file
/// descriptors, the offset is also changed for the other file descriptor.
///
/// The two file descriptors do not share file descriptor flags (the
/// close-on-exec flag). The close-on-exec flag (FD_CLOEXEC; see fcntl(2)) for
/// the duplicate descriptor is off.
///
/// On success, these system calls return the new file descriptor.  On error, -1
/// is returned, and errno is set to indicate the error.
pub fn sys_dup(oldfd: usize) -> SyscallResult {
    let task = current_task();
    task.with_mut_fd_table(|table| table.dup(oldfd))
}

/// # dup2()
///
/// The dup2() system call performs the same task as dup(), but instead of using
/// the lowest-numbered unused file descriptor, it uses the file descriptor
/// number specified in newfd. In other words, the file descriptor newfd is
/// adjusted so that it now refers to the same open file description as oldfd.
///
/// If the file descriptor newfd was previously open, it is closed before being
/// reused; the close is performed silently (i.e., any errors during the close
/// are not reported by dup2()).
///
/// Note the following points:
/// + If oldfd is not a valid file descriptor, then the call fails, and newfd is
///   not closed.
/// + If oldfd is a valid file descriptor, and newfd has the same value as
///   oldfd, then dup2() does nothing, and returns newfd.
///
/// # dup3()
///
/// dup3() is the same as dup2(), except that:
/// + The caller can force the close-on-exec flag to be set for the new file
///   descriptor by specifying O_CLOEXEC in flags. See the description of the
///   same flag in open(2) for reasons why this may be useful.
/// + If oldfd equals newfd, then dup3() fails with the error EINVAL.
pub fn sys_dup3(oldfd: usize, newfd: usize, flags: i32) -> SyscallResult {
    if oldfd == newfd {
        return Err(SysError::EINVAL);
    }
    // TODO: flags support
    let flags = OpenFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    let task = current_task();
    task.with_mut_fd_table(|table| table.dup3(oldfd, newfd))
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
pub fn at_helper(
    fd: isize,
    path: &str,
    flags: OpenFlags,
    mode: InodeMode,
) -> SysResult<Arc<dyn Dentry>> {
    log::info!("[at_helper] fd: {fd}, path: {path}");
    let task = current_task();
    let path = if is_absolute_path(path) {
        Path::new(sys_root_dentry(), sys_root_dentry(), path)
    } else if fd as i32 == AT_FDCWD {
        Path::new(sys_root_dentry(), task.cwd(), path)
    } else {
        let fd = fd as usize;
        let file = task.with_fd_table(|table| table.get(fd))?;
        Path::new(sys_root_dentry(), file.dentry(), path)
    };
    path.walk(flags, mode)
}

/// Given a path, absolute or relative, will find.
pub fn resolve_path(path: &str) -> SysResult<Arc<dyn Dentry>> {
    at_helper(
        AT_FDCWD as isize,
        path,
        OpenFlags::empty(),
        InodeMode::empty(),
    )
}
