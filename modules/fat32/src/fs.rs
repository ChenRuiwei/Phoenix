use alloc::{collections::BTreeMap, sync::Arc};

use vfs_core::{
    DentryMeta, FileSystemType, FileSystemTypeMeta, InodeMode, SuperBlock, SuperBlockMeta,
};

use crate::{dentry::FatDentry, inode::dir::FatDirInode, DiskCursor, FatFs, Mutex};

pub struct FatFsType {
    meta: FileSystemTypeMeta,
}

impl FatFsType {
    pub fn new() -> FatFsType {
        Self {
            meta: FileSystemTypeMeta::new("fat32"),
        }
    }
}

impl FileSystemType for FatFsType {
    fn meta(&self) -> &FileSystemTypeMeta {
        &self.meta
    }

    fn mount(
        self: &Arc<Self>,
        abs_mnt_path: &str,
        flags: vfs_core::MountFlags,
        dev: Option<Arc<dyn driver::BlockDevice>>,
    ) -> systype::SysResult<Arc<dyn vfs_core::Dentry>> {
        let dev = dev.unwrap();
        let sb = FatSuperBlock::new(SuperBlockMeta::new(dev, self.clone()));
        let root_inode = FatDirInode::new(sb.clone(), sb.fs.root_dir());
        let root_dentry =
            FatDentry::new(DentryMeta::new(abs_mnt_path, sb.clone(), root_inode, None));
        sb.set_root_dentry(root_dentry.clone());
        self.insert_sb(abs_mnt_path, sb);
        Ok(root_dentry)
    }

    fn kill_sb(&self, sb: Arc<dyn vfs_core::SuperBlock>) -> systype::SysResult<()> {
        todo!()
    }
}

pub struct FatSuperBlock {
    meta: SuperBlockMeta,
    fs: FatFs,
}

impl FatSuperBlock {
    pub fn new(meta: SuperBlockMeta) -> Arc<Self> {
        let blk_dev = meta.device.clone();
        Arc::new(Self {
            meta,
            fs: FatFs::new(
                DiskCursor {
                    sector: 0,
                    offset: 0,
                    blk_dev,
                },
                fatfs::FsOptions::new(),
            )
            .unwrap(),
        })
    }
}

impl SuperBlock for FatSuperBlock {
    fn meta(&self) -> &SuperBlockMeta {
        &self.meta
    }

    fn fs_stat(&self) -> systype::SysResult<vfs_core::StatFs> {
        todo!()
    }

    fn sync_fs(&self, wait: isize) -> systype::SysResult<()> {
        todo!()
    }
}
