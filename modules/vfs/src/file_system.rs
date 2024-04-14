use alloc::{string::String, sync::Arc};

use systype::SysResult;

use crate::{dentry::Dentry, inode::Inode, super_block::SuperBlock};

pub trait FileSystem: Send + Sync {
    // 文件系统单例模式

    // 挂载文件系统（会获取单例所有权）
    fn mount(
        self: Arc<Self>,
        flags: u32,
        absolute_mount_point: &str,
        device: Option<Arc<dyn Inode>>,
        data: &[u8],
    ) -> SysResult<Arc<dyn Dentry>>;

    // 卸载文件系统
    fn unmount(&self, super_block: Arc<dyn SuperBlock>) -> SysResult<()>;

    // 获取文件系统名称
    fn get_fs_name(&self) -> String;
}

impl dyn FileSystem {
    // 挂载文件系统（不获取所有权）
    pub fn mount_weak(
        self: &Arc<Self>,
        flags: u32,
        absolute_mount_point: &str,
        device: Option<Arc<dyn Inode>>,
        data: &[u8],
    ) -> SysResult<Arc<dyn Dentry>> {
        self.clone()
            .mount(flags, absolute_mount_point, device, data)
    }
}
