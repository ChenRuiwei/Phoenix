use alloc::{string::String, vec::Vec};

use systype::{SysError, SysResult};

use crate::{
    file::VFSFile,
    stat::VFSStat,
    super_block::VFSSuperBlock,
    utils::{VFSFileStat, VFSNodePermission, VFSNodeType, VFSRenameFlag, VFSTime, VFSTimeSpec},
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
    pub atime: VFSTimeSpec, // 最后访问时间
    pub mtime: VFSTimeSpec, // 最后修改时间
    pub ctime: VFSTimeSpec, // 最后改变时间
}

pub trait VFSInode: VFSFile {
    // 获取所在文件系统的超级块
    fn get_super_block(&self) -> SysResult<Arc<dyn VFSSuperBlock>> {
        Err(SysError::ENOSYS)
    }

    /// Get the permission of this inode
    fn get_node_perm(&self) -> VFSNodePermission {
        VFSNodePermission::empty()
    }

    /// Create a new node with the given `path` in the directory
    fn create(
        &self,
        _name: &str,
        _ty: VFSNodeType,
        _perm: VFSNodePermission,
        _rdev: Option<u64>,
    ) -> SysResult<Arc<dyn VFSInode>> {
        Err(SysError::ENOSYS)
    }

    /// Create a new hard link to the src dentry
    fn link(&self, _name: &str, _src: Arc<dyn VFSInode>) -> SysResult<Arc<dyn VFSInode>> {
        Err(SysError::ENOSYS)
    }

    /// Remove hard link of file `name` from dir directory
    fn unlink(&self, _name: &str) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    /// Create a new symbolic link to the \[syn_name] file
    fn symlink(&self, _name: &str, _sy_name: &str) -> SysResult<Arc<dyn VFSInode>> {
        Err(SysError::ENOSYS)
    }

    fn lookup(&self, _name: &str) -> SysResult<Arc<dyn VFSInode>> {
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
    fn get_attr(&self) -> SysResult<VFSFileStat> {
        Err(SysError::ENOSYS)
    }

    /// Called by the VFS to list all extended attributes for a given file.
    ///
    /// This method is called by the listxattr(2) system call.
    fn list_xattr(&self) -> SysResult<Vec<String>> {
        Err(SysError::ENOSYS)
    }

    fn inode_type(&self) -> VFSNodeType;

    fn truncate(&self, _len: u64) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    /// Rename the file `old_name` to `new_name` in the directory `new_parent`.
    fn rename_to(
        &self,
        _old_name: &str,
        _new_parent: Arc<dyn VFSInode>,
        _new_name: &str,
        _flag: VFSRenameFlag,
    ) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }

    /// Update the access and modification times of the inode.
    ///
    /// This method is called by the utimensat(2) system call. The ctime will be
    /// updated automatically.
    ///
    /// The parameter `now` is used to update ctime.
    fn update_time(&self, _time: VFSTime, _now: VFSTimeSpec) -> SysResult<()> {
        Err(SysError::ENOSYS)
    }
}
