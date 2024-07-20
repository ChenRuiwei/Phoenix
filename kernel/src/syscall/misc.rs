//! Miscellaneous system calls

use core::mem::size_of;

use arch::time::get_time_duration;
use systype::SyscallResult;

use super::Syscall;
use crate::mm::UserWritePtr;

// Defined in <sys/utsname.h>.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct UtsName {
    /// Name of the implementation of the operating system.
    pub sysname: [u8; 65],
    /// Name of this node on the network.
    pub nodename: [u8; 65],
    /// Current release level of this implementation.
    pub release: [u8; 65],
    /// Current version level of this release.
    pub version: [u8; 65],
    /// Name of the hardware type the system is running on.
    pub machine: [u8; 65],
    /// Name of the domain of this node on the network.
    pub domainname: [u8; 65],
}

impl UtsName {
    // TODO: Is the default value copied from Titanix correct?
    pub fn default() -> Self {
        Self {
            sysname: Self::from_str("Linux"),
            nodename: Self::from_str("Linux"),
            release: Self::from_str("5.19.0-42-generic"),
            version: Self::from_str(
                "#43~22.04.1-Ubuntu SMP PREEMPT_DYNAMIC Fri Apr 21 16:51:08 UTC 2",
            ),
            machine: Self::from_str("RISC-V SiFive Freedom U740 SoC"),
            domainname: Self::from_str("localhost"),
        }
    }

    fn from_str(info: &str) -> [u8; 65] {
        let mut data: [u8; 65] = [0; 65];
        data[..info.len()].copy_from_slice(info.as_bytes());
        data
    }
}
pub const SYSINFO_SIZE: usize = size_of::<Sysinfo>();

const _F_SIZE: usize = 20 - 2 * size_of::<u64>() - size_of::<u32>();

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Sysinfo {
    /// Seconds since boot
    pub uptime: i64,
    /// 1, 5, and 15 minute load averages
    pub loads: [u64; 3],
    /// Total usable main memory size
    pub totalram: u64,
    /// Available memory size
    pub freeram: u64,
    /// Amount of shared memory
    pub sharedram: u64,
    /// Memory used by buffers
    pub bufferram: u64,
    /// Total swap space size
    pub totalswap: u64,
    /// swap space still available
    pub freeswap: u64,
    /// Number of current processes
    pub procs: u16,
    /// Explicit padding for m68k
    pub pad: u16,
    /// Total high memory size
    pub totalhigh: u64,
    /// Available high memory size
    pub freehigh: u64,
    /// Memory unit size in bytes
    pub mem_uint: u32,
    /// Padding: libc5 uses this..
    pub _f: [u8; _F_SIZE],
}

impl Sysinfo {
    pub fn collect() -> Self {
        Self {
            uptime: get_time_duration().as_secs() as i64,
            loads: [0; 3],
            totalram: 0,
            freeram: 0,
            sharedram: 0,
            bufferram: 0,
            totalswap: 0,
            freeswap: 0,
            procs: 0,
            pad: 0,
            totalhigh: 0,
            freehigh: 0,
            mem_uint: 0,
            _f: [0; _F_SIZE],
        }
    }
}

impl Syscall<'_> {
    /// uname() returns system information in the structure pointed to by buf.
    pub fn sys_uname(&self, buf: UserWritePtr<UtsName>) -> SyscallResult {
        let task = self.task;
        buf.write(&task, UtsName::default())?;
        Ok(0)
    }

    pub fn sys_syslog(&self, log_type: usize, bufp: UserWritePtr<u8>, len: usize) -> SyscallResult {
        let task = self.task;
        log::warn!("[sys_log] unimplemeted");
        match log_type {
            2 | 3 | 4 => {
                // For type equal to 2, 3, or 4, a successful call to syslog() returns the
                // number of bytes read.
                bufp.into_mut_slice(&task, len)?;
                Ok(0)
            }
            9 => {
                // For type 9, syslog() returns the number of bytes currently available to be
                // read on the kernel log buffer.
                Ok(0)
            }
            10 => {
                // For type 10, syslog() returns the total size of the kernel log buffer.  For
                // other values of type, 0 is returned on success.
                Ok(0)
            }
            _ => {
                // For other values of type, 0 is returned on success.
                Ok(0)
            }
        }
    }

    pub fn sys_sysinfo(&self, info: UserWritePtr<Sysinfo>) -> SyscallResult {
        info.write(self.task, Sysinfo::collect())?;
        Ok(0)
    }
}
