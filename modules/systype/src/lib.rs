#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use core::{future::Future, pin::Pin};

use strum::FromRepr;
use time::timeval::TimeVal;

pub type SyscallResult = Result<usize, SysError>;
pub type SysResult<T> = Result<T, SysError>;

pub type SysFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub type ASyscallResult<'a> = SysFuture<'a, SyscallResult>;
pub type ASysResult<'a, T> = SysFuture<'a, SysResult<T>>;

/// Linux specific error codes defined in `errno.h`.
/// Defined in <asm-generic/errno-base.h> and <asm-generic/errno.h>.
/// https://www.man7.org/linux/man-pages/man3/errno.3.html
/// https://elixir.bootlin.com/linux/v6.8.9/source/include/uapi/asm-generic/errno.h#L71
#[derive(FromRepr, Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum SysError {
    /// Operation not permitted
    EPERM = 1,
    /// No such file or directory
    ENOENT = 2,
    /// No such process
    ESRCH = 3,
    /// Interrupted system call
    EINTR = 4,
    /// I/O error
    EIO = 5,
    /// No such device or address
    ENXIO = 6,
    /// Argument list too long
    E2BIG = 7,
    /// Exec format error
    ENOEXEC = 8,
    /// Bad file number
    EBADF = 9,
    /// No child processes
    ECHILD = 10,
    /// Resource temporarily unavailable
    EAGAIN = 11,
    /// Out of memory
    ENOMEM = 12,
    /// Permission denied
    EACCES = 13,
    /// Bad address
    EFAULT = 14,
    /// Block device required
    ENOTBLK = 15,
    /// Device or resource busy
    EBUSY = 16,
    /// File exists
    EEXIST = 17,
    /// Cross-device link
    EXDEV = 18,
    /// No such device
    ENODEV = 19,
    /// Not a directory
    ENOTDIR = 20,
    /// Is a directory
    EISDIR = 21,
    /// Invalid argument
    EINVAL = 22,
    /// File table overflow
    ENFILE = 23,
    /// Too many open files
    EMFILE = 24,
    /// Not a typewriter
    ENOTTY = 25,
    /// Text file busy
    ETXTBSY = 26,
    /// File too large
    EFBIG = 27,
    /// No space left on device
    ENOSPC = 28,
    /// Illegal seek
    ESPIPE = 29,
    /// Read-only file system
    EROFS = 30,
    /// Too many links
    EMLINK = 31,
    /// Broken pipe
    EPIPE = 32,
    /// Math argument out of domain of func
    EDOM = 33,
    /// Math result not representable
    ERANGE = 34,
    /// Resource deadlock would occur
    EDEADLK = 35,
    /// File name too long
    ENAMETOOLONG = 36,
    /// No record locks available
    ENOLCK = 37,
    /// Invalid system call number
    ENOSYS = 38,
    /// Directory not empty
    ENOTEMPTY = 39,
    /// Socket operation on non-socket
    ENOTSOCK = 88,
    /// Unsupported
    EOPNOTSUPP = 95,
    /// Socket address is already in use
    EADDRINUSE = 98,
    /// Address not available
    EADDRNOTAVAIL = 99,
    /// Connection reset
    ECONNRESET = 104,
    /// The socket is not connected
    ENOTCONN = 107,
    /// Connection refused
    ECONNREFUSED = 111,
}

impl SysError {
    /// Returns the error description.
    pub const fn as_str(&self) -> &'static str {
        use self::SysError::*;
        match self {
            EPERM => "Operation not permitted",
            ENOENT => "No such file or directory",
            ESRCH => "No such process",
            EINTR => "Interrupted system call",
            EIO => "I/O error",
            ENXIO => "No such device or address",
            E2BIG => "Argument list too long",
            ENOEXEC => "Exec format error",
            EBADF => "Bad file number",
            ECHILD => "No child processes",
            EAGAIN => "Try again",
            ENOMEM => "Out of memory",
            EACCES => "Permission denied",
            EFAULT => "Bad address",
            ENOTBLK => "Block device required",
            EBUSY => "Device or resource busy",
            EEXIST => "File exists",
            EXDEV => "Cross-device link",
            ENODEV => "No such device",
            ENOTDIR => "Not a directory",
            EISDIR => "Is a directory",
            EINVAL => "Invalid argument",
            ENFILE => "File table overflow",
            EMFILE => "Too many open files",
            ENOTTY => "Not a typewriter",
            ETXTBSY => "Text file busy",
            EFBIG => "File too large",
            ENOSPC => "No space left on device",
            ESPIPE => "Illegal seek",
            EROFS => "Read-only file system",
            EMLINK => "Too many links",
            EPIPE => "Broken pipe",
            EDOM => "Math argument out of domain of func",
            ERANGE => "Math result not representable",
            EDEADLK => "Resource deadlock would occur",
            ENAMETOOLONG => "File name too long",
            ENOLCK => "No record locks available",
            ENOSYS => "Invalid system call number",
            ENOTEMPTY => "Directory not empty",
            ENOTSOCK => "Socket operation on non-socket",
            ENOTCONN => "Transport endpoint is not connected",
            EOPNOTSUPP => "Unsupported Error",
            EADDRNOTAVAIL => "Address not available",
            EADDRINUSE => "Address already in use",
            ECONNRESET => "Connection reset",
            ECONNREFUSED => "Connection refused",
        }
    }

    pub fn from_i32(value: i32) -> Self {
        Self::from_repr(value).unwrap()
    }

    /// Returns the error code value in `i32`.
    pub const fn code(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Rusage {
    pub utime: TimeVal, // This is the total amount of time spent executing in user mode
    pub stime: TimeVal, // This is the total amount of time spent executing in kernel mode
    pub maxrss: usize,  // maximum resident set size
    pub ixrss: usize,   // In modern systems, this field is usually no longer used
    pub idrss: usize,   // In modern systems, this field is usually no longer used
    pub isrss: usize,   // In modern systems, this field is usually no longer used
    pub minflt: usize,  // page reclaims (soft page faults)
    pub majflt: usize,  // page faults (hard page faults)
    pub nswap: usize,   // swaps
    pub inblock: usize, // block input operations
    pub oublock: usize, // block output operations
    pub msgsnd: usize,  // In modern systems, this field is usually no longer used
    pub msgrcv: usize,  // In modern systems, this field is usually no longer used
    pub nsignals: usize, // In modern systems, this field is usually no longer used
    pub nvcsw: usize,   // voluntary context switches
    pub nivcsw: usize,  // involuntary context switches
}

pub const RLIM_INFINITY: usize = usize::MAX;

/// Resource Limit
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RLimit {
    /// Soft limit: the kernel enforces for the corresponding resource
    pub rlim_cur: usize,
    /// Hard limit (ceiling for rlim_cur)
    pub rlim_max: usize,
}

impl RLimit {
    pub fn new(rlim_cur: usize) -> Self {
        Self {
            rlim_cur,
            rlim_max: RLIM_INFINITY,
        }
    }
}
