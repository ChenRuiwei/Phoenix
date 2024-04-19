//! # Syscall numbers
//!
//! Syscall numbers from "asm-generic/unistd.h".

#![allow(unused)]

pub(super) const SYSCALL_GETCWD: usize = 17;
pub(super) const SYSCALL_DUP: usize = 23;
pub(super) const SYSCALL_DUP3: usize = 24;
pub(super) const SYSCALL_FCNTL: usize = 25;
pub(super) const SYSCALL_IOCTL: usize = 29;
pub(super) const SYSCALL_UNLINK: usize = 35;
pub(super) const SYSCALL_MKNOD: usize = 33;
pub(super) const SYSCALL_MKDIR: usize = 34;
pub(super) const SYSCALL_UMOUNT: usize = 39;
pub(super) const SYSCALL_MOUNT: usize = 40;
pub(super) const SYSCALL_STATFS: usize = 43;
pub(super) const SYSCALL_FTRUNCATE: usize = 46;
pub(super) const SYSCALL_FACCESSAT: usize = 48;
pub(super) const SYSCALL_CHDIR: usize = 49;
pub(super) const SYSCALL_FCHMODAT: usize = 53;
pub(super) const SYSCALL_OPEN: usize = 56;
pub(super) const SYSCALL_CLOSE: usize = 57;
pub(super) const SYSCALL_PIPE: usize = 59;
pub(super) const SYSCALL_GETDENTS: usize = 61;
pub(super) const SYSCALL_LSEEK: usize = 62;
pub(super) const SYSCALL_READ: usize = 63;
pub(super) const SYSCALL_WRITE: usize = 64;
pub(super) const SYSCALL_READV: usize = 65;
pub(super) const SYSCALL_WRITEV: usize = 66;
pub(super) const SYSCALL_PREAD64: usize = 67;
pub(super) const SYSCALL_PWRITE64: usize = 68;
pub(super) const SYSCALL_SENDFILE: usize = 71;
pub(super) const SYSCALL_PSELECT6: usize = 72;
pub(super) const SYSCALL_PPOLL: usize = 73;
pub(super) const SYSCALL_READLINKAT: usize = 78;
pub(super) const SYSCALL_NEWFSTATAT: usize = 79;
pub(super) const SYSCALL_FSTAT: usize = 80;
pub(super) const SYSCALL_SYNC: usize = 81;
pub(super) const SYSCALL_FSYNC: usize = 82;
pub(super) const SYSCALL_UTIMENSAT: usize = 88;
pub(super) const SYSCALL_EXIT: usize = 93;
pub(super) const SYSCALL_EXIT_GROUP: usize = 94;
pub(super) const SYSCALL_SET_TID_ADDRESS: usize = 96;
pub(super) const SYSCALL_FUTEX: usize = 98;
pub(super) const SYSCALL_SET_ROBUST_LIST: usize = 99;
pub(super) const SYSCALL_GET_ROBUST_LIST: usize = 100;
pub(super) const SYSCALL_NANOSLEEP: usize = 101;
pub(super) const SYSCALL_SETITIMER: usize = 103;
pub(super) const SYSCALL_CLOCK_SETTIME: usize = 112;
pub(super) const SYSCALL_CLOCK_GETTIME: usize = 113;
pub(super) const SYSCALL_CLOCK_GETRES: usize = 114;
pub(super) const SYSCALL_CLOCK_NANOSLEEP: usize = 115;
pub(super) const SYSCALL_SYSLOG: usize = 116;
pub(super) const SYSCALL_SCHED_SETSCHEDULER: usize = 119;
pub(super) const SYSCALL_SCHED_GETSCHEDULER: usize = 120;
pub(super) const SYSCALL_SCHED_GETPARAM: usize = 121;
pub(super) const SYSCALL_SCHED_SETAFFINITY: usize = 122;
pub(super) const SYSCALL_SCHED_GETAFFINITY: usize = 123;
pub(super) const SYSCALL_SCHED_YIELD: usize = 124;
pub(super) const SYSCALL_KILL: usize = 129;
pub(super) const SYSCALL_TKILL: usize = 130;
pub(super) const SYSCALL_TGKILL: usize = 131;
pub(super) const SYSCALL_RT_SIGSUSPEND: usize = 133;
pub(super) const SYSCALL_RT_SIGACTION: usize = 134;
pub(super) const SYSCALL_RT_SIGPROCMASK: usize = 135;
pub(super) const SYSCALL_RT_SIGTIMEDWAIT: usize = 137;
pub(super) const SYSCALL_RT_SIGRETURN: usize = 139;
pub(super) const SYSCALL_TIMES: usize = 153;
pub(super) const SYSCALL_SETPGID: usize = 154;
pub(super) const SYSCALL_GETPGID: usize = 155;
pub(super) const SYSCALL_SETSID: usize = 157;
pub(super) const SYSCALL_UNAME: usize = 160;
pub(super) const SYSCALL_GETRUSAGE: usize = 165;
pub(super) const SYSCALL_UMASK: usize = 166;
pub(super) const SYSCALL_GETTIMEOFDAY: usize = 169;
pub(super) const SYSCALL_GETPID: usize = 172;
pub(super) const SYSCALL_GETPPID: usize = 173;
pub(super) const SYSCALL_GETUID: usize = 174;
pub(super) const SYSCALL_GETEUID: usize = 175;
pub(super) const SYSCALL_GETGID: usize = 176;
pub(super) const SYSCALL_GETEGID: usize = 177;
pub(super) const SYSCALL_GETTID: usize = 178;
pub(super) const SYSCALL_SYSINFO: usize = 179;
pub(super) const SYSCALL_SHMGET: usize = 194;
pub(super) const SYSCALL_SHMCTL: usize = 195;
pub(super) const SYSCALL_SHMAT: usize = 196;
pub(super) const SYSCALL_SOCKET: usize = 198;
pub(super) const SYSCALL_SOCKETPAIR: usize = 199;
pub(super) const SYSCALL_BIND: usize = 200;
pub(super) const SYSCALL_LISTEN: usize = 201;
pub(super) const SYSCALL_ACCEPT: usize = 202;
pub(super) const SYSCALL_CONNECT: usize = 203;
pub(super) const SYSCALL_GETSOCKNAME: usize = 204;
pub(super) const SYSCALL_GETPEERNAME: usize = 205;
pub(super) const SYSCALL_SENDTO: usize = 206;
pub(super) const SYSCALL_RECVFROM: usize = 207;
pub(super) const SYSCALL_SETSOCKOPT: usize = 208;
pub(super) const SYSCALL_GETSOCKOPT: usize = 209;
pub(super) const SYSCALL_SHUTDOWN: usize = 210;
pub(super) const SYSCALL_BRK: usize = 214;
pub(super) const SYSCALL_MUNMAP: usize = 215;
pub(super) const SYSCALL_CLONE: usize = 220;
pub(super) const SYSCALL_EXECVE: usize = 221;
pub(super) const SYSCALL_MMAP: usize = 222;
pub(super) const SYSCALL_MPROTECT: usize = 226;
pub(super) const SYSCALL_MSYNC: usize = 227;
pub(super) const SYSCALL_MADVISE: usize = 233;
pub(super) const SYSCALL_WAIT4: usize = 260;
pub(super) const SYSCALL_PRLIMIT64: usize = 261;
pub(super) const SYSCALL_REMANEAT2: usize = 276;
pub(super) const SYSCALL_GETRANDOM: usize = 278;
pub(super) const SYSCALL_MEMBARRIER: usize = 283;
pub(super) const SYSCALL_COPY_FILE_RANGE: usize = 285;