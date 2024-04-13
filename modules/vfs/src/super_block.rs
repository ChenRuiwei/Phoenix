use alloc::sync::Arc;

use systype::SysResult;

use crate::{file_system::VFSFileSystem, inode::VFSInode, stat::VFSStat};

pub trait VFSSuperBlock: Send + Sync {
    // 回写同步
    // 第二个参数指示是否应该等待其他回写操作结束再执行
    fn sync_super_block(&self, _wait: bool) -> SysResult<()> {
        Ok(())
    }
    // 获取文件系统信息stat
    fn get_stat(&self) -> SysResult<VFSStat>;
    // 获取超级块对应文件系统实例
    fn get_fs(&self) -> Arc<dyn VFSFileSystem>;
    // 获取超级块对应的根inode
    fn get_root_inode(&self) -> SysResult<Arc<dyn VFSInode>>;
}
