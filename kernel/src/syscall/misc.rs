//! Miscellaneous system calls

use systype::SyscallResult;

use crate::{mm::UserWritePtr, processor::hart::current_task};

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

/// uname() returns system information in the structure pointed to by buf.
pub fn sys_uname(buf: UserWritePtr<UtsName>) -> SyscallResult {
    buf.write(current_task(), UtsName::default())?;
    Ok(0)
}
