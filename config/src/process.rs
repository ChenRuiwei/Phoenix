use crate::mm::PAGE_SIZE;

/// Syscall string arg's max length
pub const SYSCALL_STR_ARG_MAX_LEN: usize = 4096;

/// Init proc's pid
pub const INIT_PROC_PID: usize = 1;

pub const USER_STACK_SIZE: usize = 8 * 1024 * 1024;
pub const USER_STACK_PRE_ALLOC_SIZE: usize = 4 * PAGE_SIZE;
