use alloc::sync::Arc;

use device_core::BlockDevice;
use lwext4_rust::{Ext4BlockWrapper, InodeTypes};
use systype::SysResult;
use vfs_core::{
    Dentry, FileSystemType, FileSystemTypeMeta, MountFlags, StatFs, SuperBlock, SuperBlockMeta,
};

use crate::{disk::Disk, Ext4Dentry, Ext4Inode, LwExt4File};

pub struct Ext4FsType {
    meta: FileSystemTypeMeta,
}

impl Ext4FsType {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: FileSystemTypeMeta::new("ext4"),
        })
    }
}

impl FileSystemType for Ext4FsType {
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
        debug_assert!(dev.is_some());
        let sb = Ext4SuperBlock::new(SuperBlockMeta::new(dev, self.clone()));
        let mut root_ext4_file = LwExt4File::new("/", InodeTypes::EXT4_DE_DIR);
        let root_inode = Ext4Inode::new(sb.clone(), root_ext4_file);
        let root_dentry = Ext4Dentry::new(name, sb.clone(), parent.clone()).into_dyn();
        root_dentry.set_inode(root_inode);
        if let Some(parent) = parent {
            parent.insert(root_dentry.clone());
        }
        sb.set_root_dentry(root_dentry.clone());
        self.insert_sb(&root_dentry.path(), sb);
        Ok(root_dentry)
    }

    fn kill_sb(&self, _sb: Arc<dyn SuperBlock>) -> SysResult<()> {
        todo!()
    }
}

pub struct Ext4SuperBlock {
    meta: SuperBlockMeta,
    inner: Ext4BlockWrapper<Disk>,
}

unsafe impl Send for Ext4SuperBlock {}
unsafe impl Sync for Ext4SuperBlock {}

impl Ext4SuperBlock {
    pub fn new(meta: SuperBlockMeta) -> Arc<Self> {
        let blk_dev = meta.device.as_ref().unwrap().clone();
        let disk = Disk::new(blk_dev);
        let inner =
            Ext4BlockWrapper::<Disk>::new(disk).expect("failed to initialize EXT4 filesystem");
        Arc::new(Self { meta, inner })
    }
}

impl SuperBlock for Ext4SuperBlock {
    fn meta(&self) -> &SuperBlockMeta {
        &self.meta
    }

    fn stat_fs(&self) -> SysResult<StatFs> {
        todo!()
    }

    fn sync_fs(&self, _wait: isize) -> systype::SysResult<()> {
        todo!()
    }
}
