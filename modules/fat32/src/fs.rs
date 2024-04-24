use alloc::{collections::BTreeMap, sync::Arc};

use vfs_core::{
    DentryMeta, FileSystemType, FileSystemTypeMeta, InodeMode, StatFs, SuperBlock, SuperBlockMeta,
};

use crate::{as_sys_err, dentry::FatDentry, inode::dir::FatDirInode, DiskCursor, FatFs, Mutex};

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

    fn mount(
        self: Arc<Self>,
        abs_mnt_path: &str,
        flags: vfs_core::MountFlags,
        dev: Option<Arc<dyn driver::BlockDevice>>,
    ) -> systype::SysResult<Arc<dyn vfs_core::Dentry>> {
        let dev = dev.unwrap();
        let sb = FatSuperBlock::new(SuperBlockMeta::new(dev, self.clone()));
        let root_inode = FatDirInode::new(sb.clone(), sb.fs.root_dir());
        // FIXME: abs_mnt_path should not passed into dentry.
        let root_dentry = FatDentry::new_with_inode(abs_mnt_path, sb.clone(), root_inode, None);
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

    fn sync_fs(&self, wait: isize) -> systype::SysResult<()> {
        todo!()
    }
}
