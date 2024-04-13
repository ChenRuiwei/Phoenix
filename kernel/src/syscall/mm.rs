use systype::SyscallResult;

/// NOTE: The actual Linux system call returns the new program break on success.
/// On failure, the system call returns the current break.
pub fn sys_brk(addr: usize) -> SyscallResult {
    todo!()
}
