use core::isize;

use arch::time::get_time_duration;
use async_utils::dyn_future;
use config::mm::{PAGE_MASK, PAGE_SIZE};
use memory::VirtAddr;
use systype::{SysError, SyscallResult};

use crate::{
    ipc::shm::{
        SharedMemory, ShmAtFlags, ShmGetFlags, SHARED_MEMORY_KEY_ALLOCATOR, SHARED_MEMORY_MANAGER,
    },
    mm::memory_space::vm_area::{MapPerm, VmArea},
    processor::hart::current_task,
};

bitflags! {
    // See in "bits/mman-linux.h"
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
// TODO: MAP_SHARED should be shared only specified by file but not mm region?
// MAP_PRIVATE use copy on write, what if other process modify the file?
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

    log::info!("[sys_mmap] prot:{prot:?}, flags:{flags:?}, perm:{perm:?}");

    if addr.is_null() && flags.contains(MmapFlags::MAP_FIXED) {
        return Err(SysError::EINVAL);
    }

    match flags.intersection(MmapFlags::MAP_TYPE_MASK) {
        MmapFlags::MAP_SHARED => {
            if flags.contains(MmapFlags::MAP_ANONYMOUS) {
                // TODO: MAP_ANONYMOUS & MAP_SHARED is not supported, May be they share this by
                // pointing to the same addr region by parent and child process
                todo!()
            } else {
                let file = task.with_fd_table(|table| table.get(fd))?;
                // PERF: lazy alloc for mmap
                let start_va = task.with_mut_memory_space(|m| {
                    m.alloc_mmap_area(length, perm, flags, file, offset)
                })?;
                Ok(start_va.bits())
            }
        }
        MmapFlags::MAP_PRIVATE => {
            if flags.contains(MmapFlags::MAP_ANONYMOUS) {
                let start_va =
                    task.with_mut_memory_space(|m| m.alloc_mmap_private_anon(perm, length))?;
                return Ok(start_va.bits());
            }
            let file = task.with_fd_table(|table| table.get(fd))?;
            let start_va = task
                .with_mut_memory_space(|m| m.alloc_mmap_area(length, perm, flags, file, offset))?;
            Ok(start_va.bits())
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
    // if !addr.is_aligned() {
    //     return Err(SysError::EINVAL);
    // }

    // let task = current_task();
    // let range = VirtAddr::from(addr)..VirtAddr::from(addr + length);
    // task.with_mut_memory_space(|m| m.unmap(range));
    Ok(0)
}

/// allocates a System V shared memory segment
///
/// shmget() returns the identifier of the System V shared memory segment
/// associated with the value of the argument key. It may be used either to
/// obtain the identifier of a previously created shared memory segment (when
/// shmflg is zero and key does not have the value IPC_PRIVATE), or to
/// create a new set.
///
/// - `key`: Key values for shared memory
/// - `size`: The size of the shared memory to be created. A new shared memory
///   segment has a size equal to the value of size rounded up to a multiple of
///   PAGE_SIZE
/// - `shmflg`: Together with `key`, determine the function of shmget
///
/// On success, a valid shared memory identifier is returned.
pub fn sys_shmget(key: usize, size: usize, shmflg: ShmGetFlags) -> SyscallResult {
    log::warn!("[sys_shmget] {key} {size} {:?}", shmflg);
    // Create a new shared memory. When it is specified, the shmflg is invalid
    const IPC_PRIVATE: usize = 0;
    let rounded_up_sz = (size + PAGE_MASK) & !PAGE_MASK;
    if key == IPC_PRIVATE {
        let new_key = SHARED_MEMORY_KEY_ALLOCATOR.lock().alloc();
        let new_shm = SharedMemory::new(rounded_up_sz, current_task().pid());
        SHARED_MEMORY_MANAGER.0.lock().insert(new_key, new_shm);
        return Ok(new_key);
    }
    let mut shm_manager = SHARED_MEMORY_MANAGER.0.lock();
    let shm = shm_manager.get(&key);
    if let Some(shm) = shm {
        // IPC_CREAT and IPC_EXCL were specified in shmflg, but a shared memory segment
        // already exists for key.
        if shmflg.contains(ShmGetFlags::IPC_CREAT | ShmGetFlags::IPC_EXCL) {
            return Err(SysError::EEXIST);
        }
        // A segment for the given key exists, but size is greater than the size of that
        // segment.
        if shm.shmid_ds.shm_segsz < size {
            return Err(SysError::EINVAL);
        }
        return Ok(key);
    }
    if shmflg.contains(ShmGetFlags::IPC_CREAT) {
        let new_shm = SharedMemory::new(rounded_up_sz, current_task().pid());
        shm_manager.insert(key, new_shm);
        return Ok(key);
    } else {
        // No segment exists for the given key, and IPC_CREAT was not specified.
        return Err(SysError::ENOENT);
    }
}

/// After creating a shared memory, if a process wants to use it, it needs to
/// attach this memory area to its own process space
///
/// - `shmid`: the return value of `sys_shmget`
/// - `shmaddr`: Shared memory mapping address (if NULL, automatically specified
///   by the system)
///
/// On success, sys_shmat() returns an address pointer to the shared memory
/// segment
pub fn sys_shmat(shmid: usize, shmaddr: usize, shmflg: ShmAtFlags) -> SyscallResult {
    log::warn!("[sys_shmat] {shmid} {shmaddr} {:?}", shmflg);
    // unaligned (i.e., not page-aligned and SHM_RND was not specified) shmaddr
    // value
    if shmaddr & PAGE_MASK != 0 && !shmflg.contains(ShmAtFlags::SHM_RND) {
        return Err(SysError::EINVAL);
    }
    let shm_va = VirtAddr::from_usize(shmaddr).rounded_down();
    let mut map_perm = MapPerm::RW;
    if shmflg.contains(ShmAtFlags::SHM_EXEC) {
        map_perm.insert(MapPerm::X);
    }
    if shmflg.contains(ShmAtFlags::SHM_RDONLY) {
        map_perm.remove(MapPerm::W);
    }
    let mut shm_manager = SHARED_MEMORY_MANAGER.0.lock();
    let shm = shm_manager.get_mut(&shmid);
    if let Some(shm) = shm {
        let task = current_task();
        let ret_addr = task.with_mut_memory_space(|m| {
            m.attach_shm(shm.shmid_ds.shm_segsz, shm_va, map_perm, &mut shm.pages)
        });
        // shm_atime is set to the current time.
        shm.shmid_ds.shm_atime = get_time_duration();
        // shm_lpid is set to the process-ID of the calling process.
        shm.shmid_ds.shm_lpid = task.pid();
        // shm_nattch is incremented by one.
        shm.shmid_ds.shm_nattch += 1;
        return Ok(ret_addr.into());
    } else {
        // Invalid shmid value
        return Err(SysError::EINVAL);
    }
}

/// When a process no longer uses a shared memory block, it should detach from
/// the shared memory block by calling the shmdt (Shared Memory Detach)
/// function. If the process that releases this memory block is the last process
/// to use it, then this memory block will be deleted. Calling exit or any exec
/// family function will automatically cause the process to detach from the
/// shared memory block.
pub fn sys_shmdt(shmaddr: usize) -> SyscallResult {
    let mut shm = SHARED_MEMORY_MANAGER.0.lock().get(&shmaddr);

    Ok(0)
}

/// sys_shmctl performs the control operation specified by cmd on the System V
/// shared memory segment whose identifier is given in shmid.
pub fn sys_shmctl(shmid: i32, cmd: i32, _buf: usize) -> SyscallResult {
    Ok(0)
}
