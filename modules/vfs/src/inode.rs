use alloc::{string::String, sync::Arc, vec::Vec};

use systype::{SysError, SysResult};

use crate::{
    file::File,
    super_block::SuperBlock,
    utils::{FileStat, NodePermission, NodeType, RenameFlag, Time, TimeSpec},
};

pub struct InodeAttr {
    /// File mode.
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    /// File size, in bytes.
    ///
    /// For truncate
    pub size: u64,
    pub atime: TimeSpec, // 最后访问时间
    pub mtime: TimeSpec, // 最后修改时间
    pub ctime: TimeSpec, // 最后改变时间
}

pub trait Inode: File {
    // 获取所在文件系统的超级块
    fn super_block(&self) -> SysResult<Arc<dyn SuperBlock>> {
        Err(SysError::ENOSYS)
    }

    /// Get the permission of this inode
    fn node_perm(&self) -> NodePermission {
        NodePermission::empty()
    }

    /// Create a new node with the given `path` in the directory
    fn create(
        &self,
        _name: &str,
        _ty: NodeType,
        _perm: NodePermission,
        _rdev: Option<u64>,
    ) -> SysResult<Arc<dyn Inode>> {
        Err(SysError::ENOSYS)
    }

    /// Create a new hard link to the src dentry
    fn link(&self, _name: &str, _src: Arc<dyn Inode>) -> SysResult<Arc<dyn Inode>> {
        Err(SysError::ENOSYS)
    }

    /// Remove hard link of file `name` from dir directory
    fn unlink(&self, _name: &str) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    /// Create a new symbolic link to the \[syn_name] file
    fn symlink(&self, _name: &str, _sy_name: &str) -> SysResult<Arc<dyn Inode>> {
        Err(SysError::ENOSYS)
    }

    fn lookup(&self, _name: &str) -> SysResult<Arc<dyn Inode>> {
        Err(SysError::ENOSYS)
    }

    fn rmdir(&self, _name: &str) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    fn readlink(&self, _buf: &mut [u8]) -> SysResult<usize> {
        Err(SysError::ENOSYS)
    }

    /// Set the attributes of the node.
    ///
    ///  This method is called by chmod(2) and related system calls.
    fn set_attr(&self, _attr: InodeAttr) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    /// Get the attributes of the node.
    ///
    /// This method is called by stat(2) and related system calls.
    fn get_attr(&self) -> SysResult<FileStat> {
        Err(SysError::ENOSYS)
    }

    /// Called by the VFS to list all extended attributes for a given file.
    ///
    /// This method is called by the listxattr(2) system call.
    fn list_xattr(&self) -> SysResult<Vec<String>> {
        Err(SysError::ENOSYS)
    }

    fn inode_type(&self) -> NodeType;

    fn truncate(&self, _len: u64) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    /// Rename the file `old_name` to `new_name` in the directory `new_parent`.
    fn rename_to(
        &self,
        _old_name: &str,
        _new_parent: Arc<dyn Inode>,
        _new_name: &str,
        _flag: RenameFlag,
    ) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    /// Update the access and modification times of the inode.
    ///
    /// This method is called by the utimensat(2) system call. The ctime will be
    /// updated automatically.
    ///
    /// The parameter `now` is used to update ctime.
    fn update_time(&self, _time: Time, _now: TimeSpec) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }
}
