// use alloc::{
//     collections::BTreeMap,
//     string::{String, ToString},
//     sync::Arc,
//     vec::Vec,
// };
//
// use driver::BLOCK_DEVICE;
// use fat32::fatfs_shim::Fat32FileSystem;
// use spin::{Lazy, Once};
// use sync::mutex::SpinNoIrqLock;
// use vfs::{FileSystem, Inode, OpenFlags};
//
// pub static FILESYSTEMS: Once<Vec<Arc<dyn FileSystem>>> = Once::new();
//
// pub static FILE_SYSTEM_MANAGER: Once<SpinNoIrqLock<FileSystemManager>> =
// Once::new();
//
// pub struct FileSystemManager {
//     /// `mount point path` -> concrete file system
//     pub fs_mgr: BTreeMap<String, Arc<dyn FileSystem>>,
// }
//
// impl FileSystemManager {
//     pub const fn new() -> Self {
//         Self {
//             fs_mgr: BTreeMap::new(),
//         }
//     }
//
//     pub fn root_fs(&self) -> Arc<dyn FileSystem> {
//         Arc::clone(self.fs_mgr.get("/").unwrap())
//     }
//
//     pub fn push(&mut self, mount_path: String, file_system: Arc<dyn
// FileSystem>) {         self.fs_mgr.insert(mount_path, file_system);
//     }
// }
//
// pub fn get_filesystem(id: usize) -> &'static Arc<dyn FileSystem> {
//     &FILESYSTEMS.get().unwrap()[id]
// }
//
// pub fn get_root_filesystem() -> Arc<dyn FileSystem> {
//     // get_filesystem(0)
//     FILE_SYSTEM_MANAGER
//         .get()
//         .unwrap()
//         .lock()
//         .fs_mgr
//         .get("/")
//         .unwrap()
//         .clone()
// }
//
// pub fn init() {
//     log::info!("fs module initialized");
//
//     let mut file_system_manager = FileSystemManager::new();
//     let fat = Fat32FileSystem::new(BLOCK_DEVICE.get().unwrap().clone());
//
//     file_system_manager.push("/".to_string(), fat);
//     // FILESYSTEMS.call_once(|| vec![fat]);
//     // FILESYSTEMS.push(fat);
//     // FILE_SYSTEM_MANAGER.lock().insert("/".to_string(), fat);
//     FILE_SYSTEM_MANAGER.call_once(||
// SpinNoIrqLock::new(file_system_manager)); }
//
// pub fn test() {
//     let mut buf = [0; 512];
//     let busybox = FILE_SYSTEM_MANAGER
//         .get()
//         .unwrap()
//         .lock()
//         .fs_mgr
//         .get("/")
//         .unwrap()
//         .root_dir()
//         .open("busybox", OpenFlags::O_RDONLY)
//         .unwrap();
//     // let busybox = get_root_filesystem()
//     //     .root_dir()
//     //     .open("busybox", OpenFlags::O_RDONLY)
//     //     .unwrap();
//     busybox.read(0, &mut buf);
//     println!("{:?}", buf)
// }
