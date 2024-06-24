use alloc::sync::Arc;

use device_core::BlockDevice;
use vfs_core::{Dentry, FileSystemType, FileSystemTypeMeta, StatFs, SuperBlock, SuperBlockMeta};

use crate::{as_sys_err, dentry::FatDentry, inode::dir::FatDirInode, DiskCursor, FatFs};

pub struct FatFsType {
    meta: FileSystemTypeMeta,
}

impl FatFsType {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: FileSystemTypeMeta::new("fat32"),
        })
    }
}

impl FileSystemType for FatFsType {
    fn meta(&self) -> &FileSystemTypeMeta {
        &self.meta
    }

    fn base_mount(
        self: Arc<Self>,
        name: &str,
        parent: Option<Arc<dyn Dentry>>,
        _flags: vfs_core::MountFlags,
        dev: Option<Arc<dyn BlockDevice>>,
    ) -> systype::SysResult<Arc<dyn vfs_core::Dentry>> {
        debug_assert!(dev.is_some());
        let sb = FatSuperBlock::new(SuperBlockMeta::new(dev, self.clone()));
        let root_inode = FatDirInode::new(sb.clone(), sb.fs.root_dir());
        let root_dentry = FatDentry::new(name, sb.clone(), parent.clone()).into_dyn();
        root_dentry.set_inode(root_inode);
        if let Some(parent) = parent {
            parent.insert(root_dentry.clone());
        }
        sb.set_root_dentry(root_dentry.clone());
        self.insert_sb(&root_dentry.path(), sb);
        Ok(root_dentry)
    }

    fn kill_sb(&self, _sb: Arc<dyn vfs_core::SuperBlock>) -> systype::SysResult<()> {
        todo!()
    }
}

pub struct FatSuperBlock {
    meta: SuperBlockMeta,
    fs: Arc<FatFs>,
}

impl FatSuperBlock {
    pub fn new(meta: SuperBlockMeta) -> Arc<Self> {
        let blk_dev = meta.device.as_ref().unwrap().clone();
        Arc::new(Self {
            meta,
            fs: Arc::new(
                FatFs::new(
                    DiskCursor {
                        sector: 0,
                        offset: 0,
                        blk_dev,
                    },
                    fatfs::FsOptions::new(),
                )
                .unwrap(),
            ),
        })
    }
}

impl SuperBlock for FatSuperBlock {
    fn meta(&self) -> &SuperBlockMeta {
        &self.meta
    }

    fn stat_fs(&self) -> systype::SysResult<vfs_core::StatFs> {
        let stat_fs = self.fs.stats().map_err(as_sys_err)?;
        let ft = self.fs.fat_type();
        let f_type = match ft {
            fatfs::FatType::Fat12 => 0x01,
            fatfs::FatType::Fat16 => 0x04,
            fatfs::FatType::Fat32 => 0x0c,
        };
        Ok(StatFs {
            f_type,
            f_bsize: stat_fs.cluster_size() as i64,
            f_blocks: stat_fs.total_clusters() as u64,
            f_bfree: stat_fs.free_clusters() as u64,
            f_bavail: stat_fs.free_clusters() as u64,
            f_files: 0,
            f_ffree: 0,
            f_fsid: [0, 0],
            f_namelen: 255,
            f_frsize: 0,
            f_flags: 0,
            f_spare: [0; 4],
        })
    }

    fn sync_fs(&self, _wait: isize) -> systype::SysResult<()> {
        todo!()
    }
}
