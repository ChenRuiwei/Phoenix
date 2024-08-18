use systype::{SysError, SyscallResult};
use vfs::devfs::urandom::RNG;

use super::Syscall;
use crate::mm::UserWritePtr;

bitflags! {
    struct GetRandomFlag: u8 {
        const GRND_NONBLOCK = 1 << 0;
        const GRND_RANDOM = 1 << 1;
    }
}

impl Syscall<'_> {
    pub fn sys_getrandom(
        &self,
        buf: UserWritePtr<u8>,
        buflen: usize,
        _flags: usize,
    ) -> SyscallResult {
        let task = self.task;
        let mut buf = buf.into_mut_slice(&task, buflen)?;
        unsafe { RNG.fill_buf(&mut buf) };
        Ok(buf.len())
    }
}
