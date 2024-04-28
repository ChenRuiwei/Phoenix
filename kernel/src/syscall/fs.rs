use alloc::{
    ffi::CString,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::mem::size_of;

use log::info;
use systype::{SysError, SysError::EINVAL, SysResult, SyscallResult};
use vfs::{sys_root_dentry, FS_MANAGER};
use vfs_core::{
    get_name, is_absolute_path, is_relative_path, Dentry, DirEnt, File, FileMeta, Inode, InodeMeta,
    InodeMode, MountFlags, OpenFlags, Path, AT_FDCWD, AT_REMOVEDIR,
};

use crate::{
    mm::{UserReadPtr, UserWritePtr},
    processor::hart::current_task,
};

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Kstat {
    /// 设备
    pub st_dev: u64,
    /// inode 编号
    pub st_ino: u64,
    /// 文件类型
    pub st_mode: u32,
    /// 硬链接数
    pub st_nlink: u32,
    /// 用户 id
    pub st_uid: u32,
    /// 用户组 id
    pub st_gid: u32,
    /// 设备号
    pub st_rdev: u64,
    _pad0: u64,
    /// 文件大小
    pub st_size: i64,
    /// 块大小
    pub st_blksize: i32,
    _pad1: i32,
    /// 块个数
    pub st_blocks: i64,
    /// 最后一次访问时间 (秒)
    pub st_atime_sec: isize,
    /// 最后一次访问时间 (纳秒)
    pub st_atime_nsec: isize,
    /// 最后一次修改时间 (秒)
    pub st_mtime_sec: isize,
    /// 最后一次修改时间 (纳秒)
    pub st_mtime_nsec: isize,
    /// 最后一次改变状态时间 (秒)
    pub st_ctime_sec: isize,
    /// 最后一次改变状态时间 (纳秒)
    pub st_ctime_nsec: isize,
}

impl Kstat {
    pub fn from_vfs_file(file: Arc<dyn Inode>) -> SysResult<Self> {
        let stat = file.get_attr()?;
        Ok(Kstat {
            st_dev: stat.st_dev,
            st_ino: stat.st_ino,
            st_mode: stat.st_mode, // 0777 permission, we don't care about permission
            st_nlink: stat.st_nlink,
            st_uid: stat.st_uid,
            st_gid: stat.st_gid,
            st_rdev: stat.st_rdev,
            _pad0: stat.__pad,
            st_size: stat.st_size as i64,
            st_blksize: stat.st_blksize as i32,
            _pad1: stat.__pad2 as i32,
            st_blocks: stat.st_blocks as i64,
            st_atime_sec: stat.st_atime.sec as isize,
            st_atime_nsec: stat.st_atime.nsec as isize,
            st_mtime_sec: stat.st_mtime.sec as isize,
            st_mtime_nsec: stat.st_mtime.nsec as isize,
            st_ctime_sec: stat.st_ctime.sec as isize,
            st_ctime_nsec: stat.st_ctime.nsec as isize,
        })
    }
}

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
    let dentry = at_helper(dirfd, &pathname, mode)?;
    if flags.contains(OpenFlags::O_CREAT) {
        // If pathname does not exist, create it as a regular file.
        let parent = dentry.parent().expect("can not be root dentry");
        parent.create(dentry.name(), InodeMode::FILE)?;
    }
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
    let dentry = at_helper(dirfd, &pathname, mode)?;
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

pub fn sys_fstat(fd: usize, stat_buf: UserWritePtr<Kstat>) -> SyscallResult {
    let task = current_task();
    let file = task.with_fd_table(|table| table.get(fd))?;
    stat_buf.write(task, Kstat::from_vfs_file(file.inode())?)?;
    Ok(0)
}

pub async fn sys_mount(
    source: UserReadPtr<u8>,
    target: UserReadPtr<u8>,
    fstype: UserReadPtr<u8>,
    flags: u32,
    data: UserReadPtr<u8>,
) -> SyscallResult {
    let task = current_task();
    let source = source.read_cstr(task)?;
    let target = target.read_cstr(task)?; // must absolute? not mentioned in man
    let fstype = fstype.read_cstr(task)?;
    if data.is_null() {
        return Err(EINVAL);
    }
    let data = data.read_cstr(task)?;
    let flags = MountFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    log::debug!(
        "[sys_mount] source:{source:?}, target:{target:?}, fstype:{fstype:?}, flags:{flags:?}, data:{data:?}",
    );

    let fs_type_name = FS_MANAGER.lock().get(&fstype).unwrap().clone().fs_name();
    let path = Path::new(sys_root_dentry(), sys_root_dentry(), "");
    // let fs_root = match fs_type_name {
    //     name @(String::from("fat32")) => {
    //         let fs_type = FS_MANAGER.lock().get(&name).unwrap().clone();
    //         let dev = if name.eq("fat32")
    //     }
    // }
    Ok(0)
}

/// On success, the number of bytes read is returned. On end of directory, 0 is
/// returned. On error, -1 is returned, and errno is set to indicate the error.
pub fn sys_getdents64(fd: usize, buf: usize, len: usize) -> SyscallResult {
    #[derive(Debug, Clone, Copy)]
    #[repr(C)]
    struct LinuxDirent64 {
        d_ino: u64,
        d_off: u64,
        d_reclen: u16,
        d_type: u8,
        // dynmaic-len cstr d_name followsing here
    }
    // NOTE: should consider C struct align. Therefore, we can not use `size_of`
    // directly, because `size_of::<LinuxDirent64>` equals 24.
    const LEN_BEFORE_NAME: usize = 19;
    let task = current_task();
    let file = task.with_fd_table(|table| table.get(fd))?;
    if let Some(dirent) = file.read_dir()? {
        log::debug!("[sys_getdents64] dirent {dirent:?}");
        let buf = UserWritePtr::<LinuxDirent64>::from(buf);
        let ret_len = LEN_BEFORE_NAME + dirent.name.len() + 1;
        let linux_dirent = LinuxDirent64 {
            d_ino: dirent.ino,
            d_off: dirent.off,
            d_reclen: ret_len as u16,
            d_type: dirent.itype as u8,
        };
        log::debug!("[sys_getdents64] linux_dirent {linux_dirent:?}");
        if ret_len > len {
            return Err(SysError::EINVAL);
        }
        let name_buf = UserWritePtr::<u8>::from(buf.as_usize() + LEN_BEFORE_NAME);
        buf.write(task, linux_dirent)?;
        name_buf.write_cstr(task, &dirent.name)?;
        Ok(ret_len)
    } else {
        Ok(0)
    }
}

/// pipe() creates a pipe, a unidirectional data channel that can be used for
/// interprocess communication. The array pipefd is used to return two file
/// descriptors referring to the ends of the pipe. pipefd[0] refers to the read
/// end of the pipe. pipefd[1] refers to the write end of the pipe. Data written
/// to the write end of the pipe is buffered by the kernel until it is read from
/// the read end of the pipe. For further details, see pipe(7).
///
/// On success, zero is returned. On error, -1 is returned, errno is set to
/// indicate the error, and pipefd is left unchanged.
///
/// On Linux (and other systems), pipe() does not modify pipefd on failure. A
/// requirement standardizing this behavior was added in POSIX.1-2008 TC2. The
/// Linux-specific pipe2() system call likewise does not modify pipefd on
/// failure.
pub fn sys_pipe2(pipefd: UserWritePtr<[u32; 2]>, flags: i32) -> SyscallResult {
    todo!()
}

/// unlink() deletes a name from the filesystem. If that name was the last link
/// to a file and no processes have the file open, the file is deleted and the
/// space it was using is made available for reuse.
///
/// If the name was the last link to a file but any processes still have the
/// file open, the file will remain in existence until the last file descriptor
/// referring to it is closed.
///
/// The unlinkat() system call operates in exactly the same way as either
/// unlink() or rmdir(2) (depending on whether or not flags includes the
/// AT_REMOVEDIR flag) except for the differences described here.
///
/// flags is a bit mask that can either be specified as 0, or by ORing together
/// flag values that control the operation of unlinkat(). Currently, only one
/// such flag is defined:
/// + AT_REMOVEDIR: By default, unlinkat() performs the equivalent of unlink()
///   on pathname. If the AT_REMOVEDIR flag is specified, it performs the
///   equivalent of rmdir(2) on pathname.
// FIXME: removal is not delayed
pub fn sys_unlinkat(dirfd: isize, pathname: UserReadPtr<u8>, flags: i32) -> SyscallResult {
    let task = current_task();
    let path = pathname.read_cstr(task)?;
    let dentry = at_helper(dirfd, &path, InodeMode::empty())?;
    let parent = dentry.parent().expect("can not remove root directory");
    if flags == AT_REMOVEDIR {
        parent.rmdir(dentry.name())
    } else {
        parent.unlink(dentry.name())
    }
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
pub fn at_helper(fd: isize, path: &str, mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
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
    path.walk(mode)
}

/// Given a path, absolute or relative, will find.
pub fn resolve_path(path: &str) -> SysResult<Arc<dyn Dentry>> {
    at_helper(AT_FDCWD as isize, path, InodeMode::empty())
}
