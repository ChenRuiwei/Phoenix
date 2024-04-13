use alloc::{string::String, sync::Arc};

use systype::SysResult;

use crate::{dentry::VFSDentry, inode::VFSInode, super_block::VFSSuperBlock};

pub trait VFSFileSystem: Send + Sync {
    // 文件系统单例模式

    // 挂载文件系统（会获取单例所有权）
    fn mount(
        self: Arc<Self>,
        flags: u32,
        absolute_mount_point: &str,
        device: Option<Arc<dyn VFSInode>>,
        data: &[u8],
    ) -> SysResult<Arc<dyn VFSDentry>>;

    // 卸载文件系统
    fn unmount(&self, super_block: Arc<dyn VFSSuperBlock>) -> SysResult<()>;

    // 获取文件系统名称
    fn get_fs_name(&self) -> String;
}

impl dyn VFSFileSystem {
    // 挂载文件系统（不获取所有权）
    pub fn mount_weak(
        self: &Arc<Self>,
        flags: u32,
        absolute_mount_point: &str,
        device: Option<Arc<dyn VFSInode>>,
        data: &[u8],
    ) -> SysResult<Arc<dyn VFSDentry>> {
        self.clone()
            .mount(flags, absolute_mount_point, device, data)
    }
}
