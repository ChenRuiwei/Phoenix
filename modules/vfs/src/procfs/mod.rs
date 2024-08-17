mod meminfo;
mod mounts;
mod self_;

use alloc::sync::Arc;

use async_utils::block_on;
use device_core::BlockDevice;
pub use self_::KernelProcIf;
use systype::SysResult;
use vfs_core::{
    Dentry, FileSystemType, FileSystemTypeMeta, InodeMode, MountFlags, SuperBlock, SuperBlockMeta,
};

use self::{
    meminfo::{MemInfoDentry, MemInfoInode},
    mounts::{MountsDentry, MountsInode},
    self_::{ExeDentry, ExeFile, ExeInode},
};
use crate::simplefs::{dentry::SimpleDentry, inode::SimpleDirInode};

pub fn init_procfs(root_dentry: Arc<dyn Dentry>) -> SysResult<()> {
    let mem_info_dentry = MemInfoDentry::new(
        "meminfo",
        root_dentry.super_block(),
        Some(root_dentry.clone()),
    );
    let mem_info_inode = MemInfoInode::new(root_dentry.super_block(), 0);
    mem_info_dentry.set_inode(mem_info_inode);
    root_dentry.insert(mem_info_dentry);

    let mounts_dentry = MountsDentry::new(
        "mounts",
        root_dentry.super_block(),
        Some(root_dentry.clone()),
    );
    let mounts_inode = MountsInode::new(root_dentry.super_block(), 0);
    mounts_dentry.set_inode(mounts_inode);
    root_dentry.insert(mounts_dentry);

    let sys_dentry: Arc<dyn Dentry> =
        SimpleDentry::new("sys", root_dentry.super_block(), Some(root_dentry.clone()));
    let sys_inode = SimpleDirInode::new(InodeMode::DIR, root_dentry.super_block(), 0);
    sys_dentry.set_inode(sys_inode);
    root_dentry.insert(sys_dentry.clone());

    let kernel_dentry = sys_dentry.create("kernel", InodeMode::DIR)?;
    let pid_max_dentry = kernel_dentry.create("pid_max", InodeMode::FILE)?;
    let pid_max_file = pid_max_dentry.open()?;
    block_on(async { pid_max_file.write("32768\0".as_bytes()).await });

    let self_dentry: Arc<dyn Dentry> =
        SimpleDentry::new("self", root_dentry.super_block(), Some(root_dentry.clone()));
    let self_inode = SimpleDirInode::new(InodeMode::DIR, root_dentry.super_block(), 0);
    self_dentry.set_inode(self_inode);
    root_dentry.insert(self_dentry.clone());

    let self_dentry: Arc<dyn Dentry> =
        SimpleDentry::new("self", root_dentry.super_block(), Some(root_dentry.clone()));
    let self_inode = SimpleDirInode::new(InodeMode::DIR, root_dentry.super_block(), 0);
    self_dentry.set_inode(self_inode);
    let exe_dentry: Arc<dyn Dentry> =
        ExeDentry::new(root_dentry.super_block(), Some(root_dentry.clone()));
    let exe_inode = ExeInode::new(root_dentry.super_block(), 0);
    exe_dentry.set_inode(exe_inode);
    self_dentry.insert(exe_dentry);

    root_dentry.insert(self_dentry.clone());

    Ok(())
}

pub struct ProcFsType {
    meta: FileSystemTypeMeta,
}

impl ProcFsType {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: FileSystemTypeMeta::new("procfs"),
        })
    }
}

impl FileSystemType for ProcFsType {
    fn meta(&self) -> &FileSystemTypeMeta {
        &self.meta
    }

    fn base_mount(
        self: Arc<Self>,
        name: &str,
        parent: Option<Arc<dyn Dentry>>,
        _flags: MountFlags,
        dev: Option<Arc<dyn BlockDevice>>,
    ) -> SysResult<Arc<dyn Dentry>> {
        let sb = ProcSuperBlock::new(dev, self.clone());
        let mount_dentry = SimpleDentry::new(name, sb.clone(), parent.clone());
        let mount_inode = SimpleDirInode::new(InodeMode::DIR, sb.clone(), 0);
        mount_dentry.set_inode(mount_inode.clone());
        if let Some(parent) = parent {
            parent.insert(mount_dentry.clone());
        }
        self.insert_sb(&mount_dentry.path(), sb);
        Ok(mount_dentry)
    }

    fn kill_sb(&self, _sb: Arc<dyn SuperBlock>) -> SysResult<()> {
        todo!()
    }
}

pub struct ProcSuperBlock {
    meta: SuperBlockMeta,
}

impl ProcSuperBlock {
    pub fn new(
        device: Option<Arc<dyn BlockDevice>>,
        fs_type: Arc<dyn FileSystemType>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: SuperBlockMeta::new(device, fs_type),
        })
    }
}

impl SuperBlock for ProcSuperBlock {
    fn meta(&self) -> &SuperBlockMeta {
        &self.meta
    }

    fn stat_fs(&self) -> SysResult<vfs_core::StatFs> {
        todo!()
    }

    fn sync_fs(&self, _wait: isize) -> SysResult<()> {
        todo!()
    }
}
