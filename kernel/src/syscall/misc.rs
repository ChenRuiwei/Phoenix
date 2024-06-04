//! Miscellaneous system calls

use systype::SyscallResult;

use super::Syscall;
use crate::mm::UserWritePtr;

// See in "sys/utsname.h"
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
}
