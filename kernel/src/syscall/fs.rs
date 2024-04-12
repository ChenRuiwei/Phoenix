use systype::SyscallResult;

use crate::{
    mm::UserReadPtr,
    processor::{env::SumGuard, hart::current_task},
};

// TODO:
pub async fn sys_write(fd: usize, buf: UserReadPtr<u8>, len: usize) -> SyscallResult {
    assert!(fd == 1);
    let buf = buf.read_array(current_task(), len)?;
    for b in buf {
        print!("{}", b as char);
    }
    Ok(0)
}
