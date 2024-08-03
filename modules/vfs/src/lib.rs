#![no_std]
#![no_main]
#![feature(format_args_nl)]
#![feature(new_uninit)]

pub mod devfs;
pub mod fd_table;
pub mod pipefs;
pub mod procfs;
pub mod simplefs;
pub mod sockfs;
mod tmpfs;

extern crate alloc;

use alloc::{collections::BTreeMap, string::String, sync::Arc};

use driver::BLOCK_DEVICE;
use memory::FrameReleaseIf;
use procfs::init_procfs;
use sockfs::SockFsType;
use spin::Once;
use sync::mutex::SpinNoIrqLock;
use vfs_core::{Dentry, DentryState, FileSystemType, InodeMode, MountFlags, Path};

use crate::{
    devfs::{init_devfs, DevFsType},
    procfs::ProcFsType,
    tmpfs::TmpFsType,
};

type Mutex<T> = SpinNoIrqLock<T>;

pub static FS_MANAGER: Mutex<BTreeMap<String, Arc<dyn FileSystemType>>> =
    Mutex::new(BTreeMap::new());

static SYS_ROOT_DENTRY: Once<Arc<dyn Dentry>> = Once::new();

// type DiskFsType = fat32::FatFsType;
type DiskFsType = ext4::Ext4FsType;

// pub const DISK_FS_NAME: &str = "fat32";
pub const DISK_FS_NAME: &str = "ext4";

fn register_all_fs() {
    let diskfs = DiskFsType::new();
    FS_MANAGER.lock().insert(diskfs.name_string(), diskfs);

    let devfs = DevFsType::new();
    FS_MANAGER.lock().insert(devfs.name_string(), devfs);

    let procfs = ProcFsType::new();
    FS_MANAGER.lock().insert(procfs.name_string(), procfs);

    let tmpfs = TmpFsType::new();
    FS_MANAGER.lock().insert(tmpfs.name_string(), tmpfs);

    let sockfs = SockFsType::new();
    FS_MANAGER.lock().insert(sockfs.name_string(), sockfs);

    log::info!("[vfs] register fs success");
}

/// Init the filesystem.
pub fn init() {
    register_all_fs();
    let diskfs = FS_MANAGER.lock().get(DISK_FS_NAME).unwrap().clone();
    log::info!("[vfs] mounting disk fs");
    let diskfs_root = diskfs
        .mount(
            "/",
            None,
            MountFlags::empty(),
            Some(BLOCK_DEVICE.get().unwrap().clone()),
        )
        .unwrap();
    // WARN: for "lmbench_all lat_sig -P 1 prot lat_sig" test
    diskfs_root
        .create(
            "lat_sig",
            InodeMode::FILE | InodeMode::OTHER_MASK | InodeMode::GROUP_MASK | InodeMode::OWNER_MASK,
        )
        .unwrap();
    // // WARN: for "lmbench_all lat_sig -P 1 prot lat_sig" test
    // diskfs_root
    //     .create(
    //         "sort.src",
    //         InodeMode::FILE | InodeMode::OTHER_MASK | InodeMode::GROUP_MASK |
    // InodeMode::OWNER_MASK,     )
    //     .unwrap();

    log::info!("[vfs] mounting dev fs");
    let devfs = FS_MANAGER.lock().get("devfs").unwrap().clone();
    let devfs_dentry = devfs
        .mount("dev", Some(diskfs_root.clone()), MountFlags::empty(), None)
        .unwrap();
    devfs_dentry.set_state(DentryState::Sync);
    init_devfs(devfs_dentry).unwrap();

    let procfs = FS_MANAGER.lock().get("procfs").unwrap().clone();
    let procfs_dentry = procfs
        .mount("proc", Some(diskfs_root.clone()), MountFlags::empty(), None)
        .unwrap();
    procfs_dentry.set_state(DentryState::Sync);
    init_procfs(procfs_dentry).unwrap();

    let tmpfs = FS_MANAGER.lock().get("tmpfs").unwrap().clone();
    let tmpfs_dentry = tmpfs
        .mount("tmp", Some(diskfs_root.clone()), MountFlags::empty(), None)
        .unwrap();
    tmpfs_dentry.set_state(DentryState::Sync);

    let sockfs = FS_MANAGER.lock().get("sockfs").unwrap().clone();
    let sockfs_dentry = sockfs
        .mount("sock", Some(diskfs_root.clone()), MountFlags::empty(), None)
        .unwrap();
    sockfs_dentry.set_state(DentryState::Sync);

    SYS_ROOT_DENTRY.call_once(|| diskfs_root);

    sys_root_dentry().open().unwrap().load_dir().unwrap();
}

pub fn sys_root_dentry() -> Arc<dyn Dentry> {
    SYS_ROOT_DENTRY.get().unwrap().clone()
}

struct FrameReleaseIfImpl;

#[crate_interface::impl_interface]
impl FrameReleaseIf for FrameReleaseIfImpl {
    fn release_frames() {
        let ltp_dentry = Path::new(sys_root_dentry(), sys_root_dentry(), "/ltp/testcases/bin/")
            .walk()
            .unwrap();
        for (_, child) in ltp_dentry.children() {
            if !child.is_negetive() {
                let inode = child.inode().unwrap();
                inode.page_cache().unwrap().clear();
                inode.set_state(vfs_core::InodeState::UnInit)
            }
        }
    }
}
