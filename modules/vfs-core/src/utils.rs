use alloc::{
    ffi::CString,
    string::String,
    sync::{Arc, Weak},
};

use crate::{FileSystemType, InodeMode, InodeType, PERMISSION_LEN};

bitflags::bitflags! {
    // Defined in <bits/fcntl-linux.h>.
    #[derive(Debug, Clone)]
    pub struct OpenFlags: i32 {
        // reserve 3 bits for the access mode
        // NOTE: bitflags do not encourage zero bit flag, we should not directly check `O_RDONLY`
        const O_RDONLY      = 0;
        const O_WRONLY      = 1;
        const O_RDWR        = 2;
        const O_ACCMODE     = 3;
        /// If pathname does not exist, create it as a regular file.
        const O_CREAT       = 0o100;
        const O_EXCL        = 0o200;
        const O_NOCTTY      = 0o400;
        const O_TRUNC       = 0o1000;
        const O_APPEND      = 0o2000;
        const O_NONBLOCK    = 0o4000;
        const O_DSYNC       = 0o10000;
        const O_SYNC        = 0o4010000;
        const O_RSYNC       = 0o4010000;
        const O_DIRECTORY   = 0o200000;
        const O_NOFOLLOW    = 0o400000;
        const O_CLOEXEC     = 0o2000000;

        const O_ASYNC       = 0o20000;
        const O_DIRECT      = 0o40000;
        const O_LARGEFILE   = 0o100000;
        const O_NOATIME     = 0o1000000;
        const O_PATH        = 0o10000000;
        const O_TMPFILE     = 0o20200000;
    }
}

impl OpenFlags {
    pub fn readable(&self) -> bool {
        !self.contains(OpenFlags::O_WRONLY) || self.contains(OpenFlags::O_RDWR)
    }

    pub fn writable(&self) -> bool {
        self.contains(OpenFlags::O_WRONLY) || self.contains(OpenFlags::O_RDWR)
    }
}

#[derive(Default, Debug, Clone, Copy)]
#[repr(C)]
pub struct StatFs {
    /// 是个 magic number，每个知名的 fs 都各有定义，但显然我们没有
    pub f_type: i64,
    /// 最优传输块大小
    pub f_bsize: i64,
    /// 总的块数
    pub f_blocks: u64,
    /// 还剩多少块未分配
    pub f_bfree: u64,
    /// 对用户来说，还有多少块可用
    pub f_bavail: u64,
    /// 总的 inode 数
    pub f_files: u64,
    /// 空闲的 inode 数
    pub f_ffree: u64,
    /// 文件系统编号，但实际上对于不同的OS差异很大，所以不会特地去用
    pub f_fsid: [i32; 2],
    /// 文件名长度限制，这个OS默认FAT已经使用了加长命名
    pub f_namelen: isize,
    /// 片大小
    pub f_frsize: isize,
    /// 一些选项，但其实也没用到
    pub f_flags: isize,
    /// 空余 padding
    pub f_spare: [isize; 4],
}

/// Directory entry.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct DirEntry {
    pub ino: u64,
    pub off: u64,
    pub itype: InodeType,
    pub name: String,
}

bitflags! {
    // See in "bits/poll.h"
    pub struct PollEvents: i16 {
        // Event types that can be polled for. These bits may be set in `events' to
        // indicate the interesting event types; they will appear in `revents' to
        // indicate the status of the file descriptor.
        /// There is data to read.
        const POLLIN = 0x001;
        /// There is urgent data to read.
        const POLLPRI = 0x002;
        ///  Writing now will not block.
        const POLLOUT = 0x004;

        // Event types always implicitly polled for. These bits need not be set in
        // `events', but they will appear in `revents' to indicate the status of the
        // file descriptor.
        /// Error condition.
        const POLLERR = 0x008;
        /// Hang up.
        const POLLHUP = 0x010;
        /// Invalid poll request.
        const POLLNVAL = 0x020;
    }
}

#[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct TimeSpec {
    pub sec: u64,
    pub nsec: u64,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
#[repr(C)]
pub struct Stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_mode: u32,
    pub st_nlink: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_rdev: u64,
    pub __pad: u64,
    pub st_size: u64,
    pub st_blksize: u32,
    pub __pad2: u32,
    pub st_blocks: u64,
    pub st_atime: TimeSpec,
    pub st_mtime: TimeSpec,
    pub st_ctime: TimeSpec,
    pub unused: u64,
}

bitflags! {
    /// renameat flag
   pub struct RenameFlag: u32 {
       /// Atomically exchange oldpath and newpath.
       /// Both pathnames must exist but may be of different type
       const RENAME_EXCHANGE = 1 << 1;
       /// Don't overwrite newpath of the rename. Return an error if newpath already exists.
       const RENAME_NOREPLACE = 1 << 0;
       /// This operation makes sense only for overlay/union filesystem implementations.
       const RENAME_WHITEOUT = 1 << 2;
   }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Time {
    AccessTime(TimeSpec),
    ModifiedTime(TimeSpec),
}

bitflags! {
    #[derive(Debug)]
    pub struct MountFlags:u32 {
        /// This filesystem is mounted read-only.
        const MS_RDONLY = 1;
        /// The set-user-ID and set-group-ID bits are ignored by exec(3) for executable files on this filesystem.
        const MS_NOSUID = 1 << 1;
        /// Disallow access to device special files on this filesystem.
        const MS_NODEV = 1 << 2;
        /// Execution of programs is disallowed on this filesystem.
        const MS_NOEXEC = 1 << 3;
        /// Writes are synched to the filesystem immediately (see the description of O_SYNC in open(2)).
        const MS_SYNCHRONOUS = 1 << 4;
        /// Alter flags of a mounted FS
        const MS_REMOUNT = 1 << 5;
        /// Allow mandatory locks on an FS
        const MS_MANDLOCK = 1 << 6;
        /// Directory modifications are synchronous
        const MS_DIRSYNC = 1 << 7;
        /// Do not follow symlinks
        const MS_NOSYMFOLLOW = 1 << 8;
        /// Do not update access times.
        const MS_NOATIME = 1 << 10;
        /// Do not update directory access times
        const MS_NODEIRATIME = 1 << 11;
        const MS_BIND = 1 << 12;
        const MS_MOVE = 1 << 13;
        const MS_REC = 1 << 14;
        /// War is peace. Verbosity is silence.
        const MS_SILENT = 1 << 15;
        /// VFS does not apply the umask
        const MS_POSIXACL = 1 << 16;
        /// change to unbindable
        const MS_UNBINDABLE = 1 << 17;
        /// change to private
        const MS_PRIVATE = 1 << 18;
        /// change to slave
        const MS_SLAVE = 1 << 19;
        /// change to shared
        const MS_SHARED = 1 << 20;
        /// Update atime relative to mtime/ctime.
        const MS_RELATIME = 1 << 21;
        /// this is a kern_mount call
        const MS_KERNMOUNT = 1 << 22;
        /// Update inode I_version field
        const MS_I_VERSION = 1 << 23;
        /// Always perform atime updates
        const MS_STRICTATIME = 1 << 24;
        /// Update the on-disk [acm]times lazily
        const MS_LAZYTIME = 1 << 25;
        /// These sb flags are internal to the kernel
        const MS_SUBMOUNT = 1 << 26;
        const MS_NOREMOTELOCK = 1 << 27;
        const MS_NOSEC = 1 << 28;
        const MS_BORN = 1 << 29;
        const MS_ACTIVE = 1 << 30;
        const MS_NOUSER = 1 << 31;
    }
}

/// Enumeration of possible methods to seek within an I/O object.
///
/// Copied from `std`.
#[derive(Copy, PartialEq, Eq, Clone, Debug)]
pub enum SeekFrom {
    /// Sets the offset to the provided number of bytes.
    Start(u64),

    /// Sets the offset to the size of this object plus the specified number of
    /// bytes.
    ///
    /// It is possible to seek beyond the end of an object, but it's an error to
    /// seek before byte 0.
    End(i64),

    /// Sets the offset to the current position plus the specified number of
    /// bytes.
    ///
    /// It is possible to seek beyond the end of an object, but it's an error to
    /// seek before byte 0.
    Current(i64),
}

/// Special value used to indicate the *at functions should use the current
/// working directory.
pub const AT_FDCWD: i32 = -100;
/// Do not follow symbolic links.
pub const AT_SYMLINK_NOFOLLOW: i32 = 0x100;
/// Remove directory instead of unlinking file.
pub const AT_REMOVEDIR: i32 = 0x200;
/// Follow symbolic links.
pub const AT_SYMLINK_FOLLOW: i32 = 0x400;
