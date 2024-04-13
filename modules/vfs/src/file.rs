use systype::{SysError, SysResult};

use crate::utils::{VFSDirEntry, VFSPollEvents};

pub trait VFSFile: Send + Sync {
    // 在offset上读
    fn read_at(&self, _offset: u64, _buf: &mut [u8]) -> SysResult<usize> {
        Err(SysError::ENOSYS)
    }

    // 在offset上写
    fn write_at(&self, _offset: u64, _buf: &[u8]) -> SysResult<usize> {
        Err(SysError::ENOSYS)
    }

    // 读取dentry，返回合法的dentry或Error
    fn read_dir(&self, _start_index: usize) -> SysResult<Option<VFSDirEntry>> {
        Err(SysError::ENOSYS)
    }

    fn poll(&self, event: VFSPollEvents) -> SysResult<VFSPollEvents> {
        let mut res = VFSPollEvents::empty();
        if event.contains(VFSPollEvents::IN) {
            res |= VFSPollEvents::IN;
        }
        if event.contains(VFSPollEvents::OUT) {
            res |= VFSPollEvents::OUT;
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
