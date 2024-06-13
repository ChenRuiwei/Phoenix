use alloc::vec::Vec;

use config::mm::PAGE_SIZE;

/// end of vector
pub const AT_NULL: usize = 0;
/// entry should be ignored
#[allow(unused)]
pub const AT_IGNORE: usize = 1;
/// file descriptor of program
#[allow(unused)]
pub const AT_EXECFD: usize = 2;
/// program headers for program
pub const AT_PHDR: usize = 3;
/// size of program header entry
pub const AT_PHENT: usize = 4;
/// number of program headers
pub const AT_PHNUM: usize = 5;
/// system page size
pub const AT_PAGESZ: usize = 6;
/// base address of interpreter
pub const AT_BASE: usize = 7;
/// flags
pub const AT_FLAGS: usize = 8;
/// entry point of program
pub const AT_ENTRY: usize = 9;
/// program is not ELF
#[allow(unused)]
pub const AT_NOTELF: usize = 10;
/// real uid
pub const AT_UID: usize = 11;
/// effective uid
pub const AT_EUID: usize = 12;
/// real gid
pub const AT_GID: usize = 13;
/// effective gid
pub const AT_EGID: usize = 14;
/// string identifying CPU for optimizations
pub const AT_PLATFORM: usize = 15;
/// arch dependent hints at CPU capabilities
pub const AT_HWCAP: usize = 16;
/// frequency at which times() increments
pub const AT_CLKTCK: usize = 17;

/// AT_* values 18 through 22 are reserved

/// secure mode boolean
pub const AT_SECURE: usize = 23;
/// string identifying real platform, may differ from AT_PLATFORM
#[allow(unused)]
pub const AT_BASE_PLATFORM: usize = 24;
/// address of 16 random bytes
// NOTE: libc may use these 16 bytes as stack check guard, therefore, the
// address must be valid
pub const AT_RANDOM: usize = 25;
/// extension of AT_HWCAP
#[allow(unused)]
pub const AT_HWCAP2: usize = 26;
/// filename of program
pub const AT_EXECFN: usize = 31;
/// entry point to the system call function in the vDSO
#[allow(unused)]
pub const AT_SYSINFO: usize = 32;
/// address of a page containing the vDSO
#[allow(unused)]
pub const AT_SYSINFO_EHDR: usize = 33;

/// Auxiliary header
#[derive(Copy, Clone)]
#[repr(C)]
pub struct AuxHeader {
    /// Type
    pub aux_type: usize,
    /// Value
    pub value: usize,
}

impl AuxHeader {
    pub fn new(aux_type: usize, value: usize) -> Self {
        Self { aux_type, value }
    }
}

pub fn generate_early_auxv(
    ph_entry_size: usize,
    ph_count: usize,
    entry_point: usize,
) -> Vec<AuxHeader> {
    let mut auxv = Vec::with_capacity(32);
    macro_rules! push {
        ($x1:expr, $x2:expr) => {
            auxv.push(AuxHeader::new($x1, $x2));
        };
    }
    push!(AT_PHENT, ph_entry_size);
    push!(AT_PHNUM, ph_count);
    push!(AT_PAGESZ, PAGE_SIZE);
    push!(AT_FLAGS, 0);
    push!(AT_ENTRY, entry_point);
    push!(AT_UID, 0);
    push!(AT_EUID, 0);
    push!(AT_GID, 0);
    push!(AT_EGID, 0);
    push!(AT_PLATFORM, 0);
    push!(AT_HWCAP, 0);
    push!(AT_CLKTCK, 100);
    push!(AT_SECURE, 0);
    auxv
}
