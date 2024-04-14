use alloc::{string::String, sync::Arc};

use systype::SysResult;

use crate::{inode::Inode, utils::MountPoint};

pub trait Dentry: Send + Sync {
    // 返回dentry名称
    fn get_name(&self) -> String;

    // 将dentry设为挂载点
    fn to_mount_point(
        self: Arc<Self>,
        sub_fs_root: Arc<dyn Dentry>,
        mount_flag: u32,
    ) -> SysResult<()>;

    // 获取dentry的inode
    fn get_inode(&self) -> SysResult<Arc<dyn Inode>>;

    /// Get the mount point of this dentry
    fn get_mount_point(&self) -> Option<MountPoint>;

    /// Remove the mount point of this dentry
    fn clear_mount_point(&self);

    /// Whether this dentry is a mount point
    fn is_mount_point(&self) -> bool {
        self.get_mount_point().is_some()
    }
    /// Lookup a dentry in the directory
    ///
    /// The dentry should cache its children to speed up the lookup
    fn find(&self, path: &str) -> Option<Arc<dyn Dentry>>;

    /// Insert a child to this dentry and return the dentry of the child
    fn insert(self: Arc<Self>, name: &str, child: Arc<dyn Inode>) -> SysResult<Arc<dyn Dentry>>;

    /// Remove a child from this dentry and return the dentry of the child
    fn remove(&self, name: &str) -> Option<Arc<dyn Dentry>>;

    /// Get the parent of this dentry
    fn get_parent(&self) -> Option<Arc<dyn Dentry>>;

    /// Set the parent of this dentry
    ///
    /// This is useful when you want to move a dentry to another directory or
    /// mount this dentry to another directory
    fn set_parent(&self, parent: &Arc<dyn Dentry>);

    /// Get the path of this dentry
    fn get_path(&self) -> String {
        if let Some(p) = self.get_parent() {
            let dentry_name = self.get_name();
            let path = if dentry_name == "/" {
                String::from("")
            } else {
                String::from("/") + dentry_name.as_str()
            };
            let parent_name = p.get_name();
            return if parent_name == "/" {
                if p.get_parent().is_some() {
                    // p is not root
                    p.get_parent().unwrap().get_path() + path.as_str()
                } else {
                    path
                }
            } else {
                // p is a mount point
                p.get_path() + path.as_str()
            };
        } else {
            String::from("/")
        }
    }
}

impl dyn Dentry {
    /// Insert a child to this dentry and return the dentry of the child
    ///
    /// It likes [`VfsDentry::insert`], but it will not take ownership of `self`
    pub fn insert_weak(
        self: &Arc<Self>,
        name: &str,
        child: Arc<dyn Inode>,
    ) -> SysResult<Arc<dyn Dentry>> {
        self.clone().insert(name, child)
    }
    /// Make this dentry to  a mount point
    ///
    /// It likes [`VfsDentry::to_mount_point`], but it will not take ownership
    /// of `self`
    pub fn to_mount_point_weak(
        self: &Arc<Self>,
        sub_fs_root: Arc<dyn Dentry>,
        mount_flag: u32,
    ) -> SysResult<()> {
        self.clone().to_mount_point(sub_fs_root, mount_flag)
    }
}
