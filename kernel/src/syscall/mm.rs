use memory::VirtAddr;
use systype::{SysError, SyscallResult};

use crate::processor::hart::current_task;

bitflags! {
    // See in "bits/mman-linux.h"
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MMAPFlags: i32 {
        // Sharing types (must choose one and only one of these).
        /// Share changes.
        const MAP_SHARED = 0x01;
        /// Changes are private.
        const MAP_PRIVATE = 0x02;
        /// Share changes and validate
        const MAP_SHARED_VALIDATE = 0x03;

        // Other flags
        /// Interpret addr exactly.
        const MAP_FIXED = 0x10;
        /// Don't use a file.
        const MAP_ANONYMOUS = 0x20;
        /// Don't check for reservations.
        const MAP_NORESERVE = 0x04000;
    }
}

/// NOTE: The actual Linux system call returns the new program break on success.
/// On failure, the system call returns the current break.
pub fn sys_brk(addr: VirtAddr) -> SyscallResult {
    let task = current_task();
    let brk = task.with_mut_memory_space(|m| m.reset_heap_break(addr));
    Ok(brk.bits())
}

pub fn sys_mmap(
    _addr: usize,
    length: usize,
    _prot: i32,
    flags: i32,
    _fd: usize,
    _offset: usize,
) -> SyscallResult {
    if length == 0 {
        return Err(SysError::EINVAL);
    }
    let _flags = MMAPFlags::from_bits(flags).ok_or(SysError::EINVAL);
    todo!()
}
