use super::mm::PAGE_SIZE;

/// Max file descriptors counts
pub const MAX_FDS: usize = 1024;

pub const PIPE_BUF_LEN: usize = 16 * PAGE_SIZE;
