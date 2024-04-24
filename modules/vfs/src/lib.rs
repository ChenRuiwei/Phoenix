#![no_std]
#![no_main]

extern crate alloc;

use alloc::{collections::BTreeMap, string::String, sync::Arc};

use driver::BLOCK_DEVICE;
use sync::mutex::SpinNoIrqLock;
use systype::SysResult;
use vfs_core::{FileSystemType, MountFlags};

type Mutex<T> = SpinNoIrqLock<T>;

pub static FS_TYPES: Mutex<BTreeMap<String, Arc<dyn FileSystemType>>> = Mutex::new(BTreeMap::new());

type DiskFsType = fat32::FatFsType;

fn register_all_fs() {
    let diskfs = DiskFsType::new();
    FS_TYPES.lock().insert(diskfs.fs_name(), diskfs);

    log::info!("[vfs] register fs success");
}

/// Init the filesystem
pub fn init_filesystem() -> SysResult<()> {
    register_all_fs();
    let diskfs = FS_TYPES.lock().get("fat32").unwrap().clone();
    let diskfs_root = diskfs.i_mount(
        "/",
        MountFlags::empty(),
        Some(BLOCK_DEVICE.get().unwrap().clone()),
    )?;
    Ok(())
}
