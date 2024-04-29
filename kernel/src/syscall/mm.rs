use async_utils::dyn_future;
use memory::VirtAddr;
use systype::{SysError, SyscallResult};

use crate::{
    mm::memory_space::vm_area::{MapPerm, VmArea},
    processor::hart::current_task,
};

bitflags! {
    // See in "bits/mman-linux.h"
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MmapFlags: i32 {
        // Sharing types (must choose one and only one of these).
        /// Share changes.
        const MAP_SHARED = 0x01;
        /// Changes are private.
        const MAP_PRIVATE = 0x02;
        /// Share changes and validate
        const MAP_SHARED_VALIDATE = 0x03;
        const MAP_TYPE_MASK = 0x03;

        // Other flags
        /// Interpret addr exactly.
        const MAP_FIXED = 0x10;
        /// Don't use a file.
        const MAP_ANONYMOUS = 0x20;
        /// Don't check for reservations.
        const MAP_NORESERVE = 0x04000;
    }
}

bitflags! {
    // See in "bits/mman-linux.h"
    // NOTE: Zero bit flag is discouraged. See https://docs.rs/bitflags/latest/bitflags/#zero-bit-flags
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MmapProt: i32 {
        /// Page can be read.
        const PROT_READ = 0x1;
        /// Page can be written.
        const PROT_WRITE = 0x2;
        /// Page can be executed.
        const PROT_EXEC = 0x4;
    }
}

impl From<MmapProt> for MapPerm {
    fn from(prot: MmapProt) -> Self {
        let mut ret = Self::U;
        if prot.contains(MmapProt::PROT_READ) {
            ret |= Self::R;
        }
        if prot.contains(MmapProt::PROT_WRITE) {
            ret |= Self::W;
        }
        if prot.contains(MmapProt::PROT_EXEC) {
            ret |= Self::X;
        }
        ret
    }
}

/// NOTE: The actual Linux system call returns the new program break on success.
/// On failure, the system call returns the current break.
pub fn sys_brk(addr: VirtAddr) -> SyscallResult {
    let task = current_task();
    let brk = task.with_mut_memory_space(|m| m.reset_heap_break(addr));
    Ok(brk.bits())
}

/// mmap() creates a new mapping in the virtual address space of the calling
/// process. The starting address for the new mapping is specified in addr. The
/// length argument specifies the length of the mapping (which must be greater
/// than 0).
///
/// If addr is NULL, then the kernel chooses the (page-aligned) address at which
/// to create the mapping; this is the most portable method of creating a new
/// mapping. If addr is not NULL, then the kernel takes it as a hint about where
/// to place the mapping; on Linux, the kernel will pick a nearby page boundary
/// (but always above or equal to the value specified by
/// /proc/sys/vm/mmap_min_addr) and attempt to create the mapping there. If
/// another mapping already exists there, the kernel picks a new address that
/// may or may not depend on the hint. The address of the new mapping is
/// returned as the result of the call.
///
/// The contents of a file mapping (as opposed to an anonymous mapping; see
/// MAP_ANONYMOUS below), are initialized using length bytes starting at offset
/// offset in the file (or other object) referred to by the file descriptor fd.
/// offset must be a multiple of the page size as returned by
/// sysconf(_SC_PAGE_SIZE).
///
/// After the mmap() call has returned, the file descriptor, fd, can be closed
/// immediately without invalidating the mapping.
///
/// On success, mmap() returns a pointer to the mapped area. On error, the value
/// MAP_FAILED (that is, (void *) -1) is returned, and errno is set to indicate
/// the error.
// NOTE: Memory mapped by mmap() is preserved across fork(2), with the same
// attributes.
pub fn sys_mmap(
    addr: VirtAddr,
    length: usize,
    prot: i32,
    flags: i32,
    fd: usize,
    offset: usize,
) -> SyscallResult {
    if length == 0 {
        return Err(SysError::EINVAL);
    }
    let task = current_task();
    let flags = MmapFlags::from_bits_truncate(flags);
    let prot = MmapProt::from_bits_truncate(prot);
    let perm = MapPerm::from(prot);

    log::debug!("[sys_mmap] prot:{prot:?}, flags:{flags:?}, perm:{perm:?}");

    if addr.is_null() && flags.contains(MmapFlags::MAP_FIXED) {
        return Err(SysError::EINVAL);
    }

    match flags.intersection(MmapFlags::MAP_TYPE_MASK) {
        MmapFlags::MAP_SHARED => {
            if flags.contains(MmapFlags::MAP_ANONYMOUS) {
                todo!()
            } else {
                let file = task.with_fd_table(|table| table.get(fd))?;
                let start_va =
                    task.with_mut_memory_space(|m| m.alloc_mmap_area(perm, length, file, offset))?;
                Ok(start_va.bits())
            }
        }
        MmapFlags::MAP_PRIVATE => {
            todo!()
        }
        _ => Err(SysError::EINVAL),
    }
}

/// The munmap() system call deletes the mappings for the specified address
/// range, and causes further references to addresses within the range to
/// generate invalid memory references. The region is also automatically
/// unmapped when the process is terminated. On the other hand, closing the file
/// descriptor does not unmap the region.
///
/// The address addr must be a multiple of the page size (but length need not
/// be). All pages containing a part of the indicated range are unmapped, and
/// subsequent references to these pages will generate SIGSEGV. It is not an
/// error if the indicated range does not contain any mapped pages.
///
/// On success, munmap() returns 0. On failure, it returns -1, and errno is
/// set to indicate the error (probably to EINVAL).
// TODO:
pub fn sys_munmap(addr: VirtAddr, length: usize) -> SyscallResult {
    Ok(0)
}
