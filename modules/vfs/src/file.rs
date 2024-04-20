use systype::{SysError, SysResult};

use crate::utils::{DirEntry, PollEvents};

pub trait File: Send + Sync {
    fn read(&self, _offset: u64, _buf: &mut [u8]) -> SysResult<usize> {
        Err(SysError::ENOSYS)
    }

    fn write(&self, _offset: u64, _buf: &[u8]) -> SysResult<usize> {
        Err(SysError::ENOSYS)
    }

    fn read_dir(&self, _start_index: usize) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOSYS)
    }

    fn poll(&self, event: PollEvents) -> SysResult<PollEvents> {
        let mut res = PollEvents::empty();
        if event.contains(PollEvents::IN) {
            res |= PollEvents::IN;
        }
        if event.contains(PollEvents::OUT) {
            res |= PollEvents::OUT;
        }
        Ok(res)
    }

    fn ioctl(&self, _cmd: u32, _arg: usize) -> SysResult<usize> {
        Err(SysError::ENOSYS)
    }

    /// Called by the close(2) system call to flush a file
    fn flush(&self) -> SysResult<()> {
        Ok(())
    }

    /// Called by the fsync(2) system call.
    fn fsync(&self) -> SysResult<()> {
        Ok(())
    }
}
