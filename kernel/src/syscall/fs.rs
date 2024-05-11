use alloc::{collections::BTreeMap, ffi::CString, sync::Arc, vec::Vec};
use core::{
    future::{self, Future},
    mem::size_of,
    pin::Pin,
    task::{Context, Poll},
};

use async_utils::{dyn_future, Async};
use driver::BLOCK_DEVICE;
use memory::VirtAddr;
use systype::{SysError, SysResult, SyscallResult};
use time::timespec::TimeSpec;
use timer::timelimited_task::{TimeLimitedTaskFuture, TimeLimitedTaskOutput};
use vfs::{pipe::new_pipe, sys_root_dentry, FS_MANAGER};
use vfs_core::{
    is_absolute_path, Dentry, Inode, InodeMode, InodeType, MountFlags, OpenFlags, Path, PollEvents,
    SeekFrom, AT_FDCWD, AT_REMOVEDIR,
};

use crate::{
    mm::{UserRdWrPtr, UserReadPtr, UserSlice, UserWritePtr},
    processor::{
        env::within_sum,
        hart::{current_task, local_hart},
    },
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
    let file = task.with_fd_table(|table| table.get(fd))?;
    let buf = buf.into_slice(&task, len)?;
    let pos = file.pos();
    let ret = file.write(pos, &buf).await?;
    file.seek(SeekFrom::Current((pos + ret) as i64));
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
    log::info!("[sys_read] reading file {}", file.dentry().path());
    let mut buf = buf.into_mut_slice(&task, len)?;
    let pos = file.pos();
    let ret = file.read(pos, &mut buf).await?;
    file.seek(SeekFrom::Current((pos + ret) as i64))?;
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
    let pathname = pathname.read_cstr(&task)?;
    log::debug!("[sys_openat] {flags:?}, {mode:?}");
    let dentry = at_helper(dirfd, &pathname, mode)?;
    if flags.contains(OpenFlags::O_CREAT) {
        // If pathname does not exist, create it as a regular file.
        if flags.contains(OpenFlags::O_EXCL) && !dentry.is_negetive() {
            return Err(SysError::EEXIST);
        }
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
    let pathname = pathname.read_cstr(&task)?;
    log::debug!("[sys_mkdirat] {mode:?}");
    let dentry = at_helper(dirfd, &pathname, mode)?;
    if !dentry.is_negetive() {
        return Err(SysError::EEXIST);
    }
    let parent = dentry.parent().unwrap();
    parent.create(dentry.name(), mode.union(InodeMode::DIR))?;
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
    buf.into_mut_slice(&task, length)?
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
    let path = path.read_cstr(&task)?;
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
// TODO: flags support
pub fn sys_dup3(oldfd: usize, newfd: usize, flags: i32) -> SyscallResult {
    if oldfd == newfd {
        return Err(SysError::EINVAL);
    }
    let flags = OpenFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    let task = current_task();
    task.with_mut_fd_table(|table| table.dup3(oldfd, newfd))
}

pub fn sys_fstat(fd: usize, stat_buf: UserWritePtr<Kstat>) -> SyscallResult {
    let task = current_task();
    let file = task.with_fd_table(|table| table.get(fd))?;
    stat_buf.write(&task, Kstat::from_vfs_file(file.inode())?)?;
    Ok(0)
}

pub fn sys_fstatat(
    dirfd: isize,
    pathname: UserReadPtr<u8>,
    stat_buf: UserWritePtr<Kstat>,
    flags: i32,
) -> SyscallResult {
    let task = current_task();
    let path = pathname.read_cstr(&task)?;
    let dentry = at_helper(dirfd, &path, InodeMode::empty())?;
    stat_buf.write(&task, Kstat::from_vfs_file(dentry.inode()?)?)?;
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
    let source = source.read_cstr(&task)?;
    let target = target.read_cstr(&task)?; // must absolute? not mentioned in man
    let fstype = fstype.read_cstr(&task)?;
    let flags = MountFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    log::debug!(
        "[sys_mount] source:{source:?}, target:{target:?}, fstype:{fstype:?}, flags:{flags:?}, data:{data:?}",
    );

    // adding this code is because the fs_type in test code is vfat, which should be
    // turned into fat32
    let fat32_type = FS_MANAGER.lock().get("fat32").unwrap().clone();
    let fs_type = FS_MANAGER
        .lock()
        .get(&fstype)
        .unwrap_or(&fat32_type.clone())
        .clone();
    let fs_root = match fs_type.name() {
        name @ ("fat32") => {
            let dev = if name.eq("fat32") {
                // here should be getting device according to inode
                // it seems that device hasn't been associated with inode yet.
                // so here just return a virtio_block
                // let path = Path::new(sys_root_dentry(), sys_root_dentry(), &*source);
                // let dev = path.walk(InodeMode::BLOCK)?;
                // let dev_ino = dev.inode()?;
                // if dev_ino.itype() != InodeType::BlockDevice {
                //     return Err(SysError::EINVAL);
                // }
                Some(BLOCK_DEVICE.get().unwrap().clone())
            } else {
                None
            };

            let abs_target = resolve_path(&target)?.path();
            fs_type.mount(&abs_target, flags, dev)?
        }
        _ => return Err(SysError::EINVAL),
    };
    // Need a mount_point struct to manage fs_root
    Ok(0)
}

pub async fn sys_umount2(target: UserReadPtr<u8>, flags: u32) -> SyscallResult {
    let task = current_task();
    let mount_path = target.read_cstr(&task)?;
    let flags = MountFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    log::info!("[sys_umount2] umount path:{mount_path:?}");
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
        // d_name follows here, which will be written later
    }
    // NOTE: Considering C struct align, we can not use `size_of` directly, because
    // `size_of::<LinuxDirent64>` equals 24, which is not what we want.
    const LEN_BEFORE_NAME: usize = 19;
    let task = current_task();
    let file = task.with_fd_table(|table| table.get(fd))?;
    let mut writen_len = 0;
    let _ = UserWritePtr::<u8>::from(buf).into_mut_slice(&task, len)?;
    while let Some(dirent) = file.read_dir()? {
        log::debug!("[sys_getdents64] dirent {dirent:?}");
        let buf = UserWritePtr::<LinuxDirent64>::from(buf + writen_len);
        let c_name_len = dirent.name.len() + 1;
        // align to 8 bytes
        let rec_len = (LEN_BEFORE_NAME + c_name_len + 7) & !0x7;
        let linux_dirent = LinuxDirent64 {
            d_ino: dirent.ino,
            d_off: dirent.off,
            d_reclen: rec_len as u16,
            d_type: dirent.itype as u8,
        };
        log::debug!("[sys_getdents64] linux dirent {linux_dirent:?}");
        if writen_len + rec_len > len {
            file.seek(SeekFrom::Current(-1))?;
            break;
        }
        let name_buf = UserWritePtr::<u8>::from(buf.as_usize() + LEN_BEFORE_NAME);
        buf.write_unchecked(&task, linux_dirent)?;
        name_buf.write_cstr_unchecked(&task, &dirent.name)?;
        writen_len += rec_len;
    }
    Ok(writen_len)
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
    let task = current_task();
    let (pipe_read, pipe_write) = new_pipe();
    let pipe = task.with_mut_fd_table(|table| {
        let fd_read = table.alloc(pipe_read)?;
        let fd_write = table.alloc(pipe_write)?;
        log::debug!("[sys_pipe2] read_fd: {fd_read}, write_fd: {fd_write}");
        Ok([fd_read as u32, fd_write as u32])
    })?;
    pipefd.write(&task, pipe)?;
    Ok(0)
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
// FIXME: removal is not delayed, could be done in vfs layer
pub fn sys_unlinkat(dirfd: isize, pathname: UserReadPtr<u8>, flags: i32) -> SyscallResult {
    let task = current_task();
    let path = pathname.read_cstr(&task)?;
    let dentry = at_helper(dirfd, &path, InodeMode::empty())?;
    let parent = dentry.parent().expect("can not remove root directory");
    if flags == AT_REMOVEDIR {
        parent.rmdir(dentry.name())
    } else {
        parent.unlink(dentry.name())
    }
}

pub fn sys_ioctl(fd: usize, cmd: usize, arg: usize) -> SyscallResult {
    let task = current_task();
    let file = task.with_fd_table(|table| table.get(fd))?;
    within_sum(|| file.ioctl(cmd, arg))
}

const F_DUPFD: i32 = 0;
const F_DUPFD_CLOEXEC: i32 = 1030;
const F_GETFD: i32 = 1;
const F_SETFD: i32 = 2;
const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;

// TODO:
pub fn sys_fcntl(fd: usize, op: i32, arg: usize) -> SyscallResult {
    let task = current_task();
    match op {
        F_DUPFD_CLOEXEC => {
            let new_fd = task.with_mut_fd_table(|table| table.dup_with_bound(fd, arg));
            new_fd
        }
        F_SETFD => {
            const FD_CLOEXEC: usize = 1;
            let file = task.with_fd_table(|table| table.get(fd))?;
            let flags = file.flags();
            if arg & FD_CLOEXEC == 0 {
                // do not close on execve
                file.set_flags(flags & !OpenFlags::O_CLOEXEC);
            } else {
                // do close on execve
                file.set_flags(flags | OpenFlags::O_CLOEXEC);
            }
            Ok(0)
        }
        _ => {
            log::warn!("fcntl cmd: {} not implemented, returning 0 as default", op);
            Ok(0)
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IoVec {
    base: usize,
    len: usize,
}

/// The writev() system call writes iovcnt buffers of data described by iov to
/// the file associated with the file descriptor fd ("gather output").
pub async fn sys_writev(fd: usize, iov: UserReadPtr<IoVec>, iovcnt: usize) -> SyscallResult {
    let task = current_task();
    let file = task.with_fd_table(|f| f.get(fd))?;
    let mut offset = file.pos();
    let mut total_len = 0;
    let iovs = iov.read_array(&task, iovcnt)?;
    for (i, iov) in iovs.iter().enumerate() {
        if iov.len == 0 {
            continue;
        }
        let ptr = UserReadPtr::<u8>::from(iov.base);
        log::debug!("[sys_writev] iov #{i}, ptr: {ptr}, len: {}", iov.len);
        let buf = ptr.into_slice(&task, iov.len)?;
        let write_len = file.write(offset, &buf).await?;
        total_len += write_len;
        offset += write_len;
    }
    file.seek(SeekFrom::Current(total_len as i64))?;
    Ok(total_len)
}

/// The readv() system call reads iovcnt buffers from the file associated with
/// the file descriptor fd into the buffers described by iov ("scatter input").
pub async fn sys_readv(fd: usize, iov: UserReadPtr<IoVec>, iovcnt: usize) -> SyscallResult {
    let task = current_task();
    let file = task.with_fd_table(|f| f.get(fd))?;
    let mut offset = file.pos();
    let mut total_len = 0;
    let iovs = iov.read_array(&task, iovcnt)?;
    for (i, iov) in iovs.iter().enumerate() {
        if iov.len == 0 {
            continue;
        }
        let ptr = UserWritePtr::<u8>::from(iov.base);
        log::debug!("[sys_readv] iov #{i}, ptr: {ptr}, len: {}", iov.len);
        let mut buf = ptr.into_mut_slice(&task, iov.len)?;
        let write_len = file.read(offset, &mut buf).await?;
        total_len += write_len;
        offset += write_len;
    }
    file.seek(SeekFrom::Current(total_len as i64))?;
    Ok(total_len)
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct PollFd {
    fd: i32,      // file descriptor
    events: i16,  // requested events
    revents: i16, // returned events
}

pub async fn sys_ppoll(
    fds: UserRdWrPtr<PollFd>,
    nfds: usize,
    timeout_ts: UserReadPtr<TimeSpec>,
    sigmask: usize,
) -> SyscallResult {
    let task = current_task();
    let fds_va: VirtAddr = fds.as_usize().into();
    let mut poll_fds = fds.read_array(&task, nfds)?;
    let timeout = if timeout_ts.is_null() {
        None
    } else {
        Some(timeout_ts.read(&task)?.into())
    };

    pub struct PollFuture<'a> {
        futures: Vec<Async<'a, SysResult<PollEvents>>>,
        ready_cnt: usize,
    }

    impl Future for PollFuture<'_> {
        type Output = Vec<(usize, SysResult<PollEvents>)>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = unsafe { self.get_unchecked_mut() };
            let mut ret_vec = Vec::new();
            for (i, future) in this.futures.iter_mut().enumerate() {
                let result = unsafe { Pin::new_unchecked(future).poll(cx) };
                if let Poll::Ready(result) = result {
                    this.ready_cnt += 1;
                    ret_vec.push((i, result))
                }
            }
            if this.ready_cnt > 0 {
                Poll::Ready(ret_vec)
            } else {
                Poll::Pending
            }
        }
    }

    let mut futures = Vec::<Async<SysResult<PollEvents>>>::with_capacity(nfds);
    for poll_fd in poll_fds.iter() {
        let fd = poll_fd.fd as usize;
        let events = PollEvents::from_bits(poll_fd.events).unwrap();
        let file = task.with_fd_table(|table| table.get(fd))?;
        let future = dyn_future(async move { file.poll(events).await });
        futures.push(future);
    }

    let poll_future = PollFuture {
        futures,
        ready_cnt: 0,
    };

    let mut poll_fds_slice = unsafe { UserSlice::<PollFd>::new_unchecked(fds_va, nfds) };

    let ret_vec = if let Some(timeout) = timeout {
        match TimeLimitedTaskFuture::new(timeout, poll_future).await {
            TimeLimitedTaskOutput::Ok(ret_vec) => ret_vec,
            TimeLimitedTaskOutput::TimeOut => {
                log::debug!("[sys_ppoll]: timeout");
                return Ok(0);
            }
        }
    } else {
        poll_future.await
    };

    let ret = ret_vec.len();
    for (i, result) in ret_vec {
        if let Ok(result) = result {
            poll_fds[i].revents |= result.bits() as i16;
        } else {
            poll_fds[i].revents |= PollEvents::POLLERR.bits() as i16;
        }
    }
    poll_fds_slice.copy_from_slice(&poll_fds);
    Ok(ret)
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
