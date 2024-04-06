use systype::SyscallResult;

use crate::processor::env::SumGuard;

// TODO:
pub async fn sys_write(fd: usize, buf: usize, len: usize) -> SyscallResult {
    assert!(fd == 1);
    let guard = SumGuard::new();
    let buf = unsafe { core::slice::from_raw_parts(buf as *const u8, len) };
    for b in buf {
        print!("{}", *b as char);
    }
    Ok(0)
}
