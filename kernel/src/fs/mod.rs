mod devfs;
pub use devfs::TTY;
pub mod fat32;
mod fd_table;
pub mod ffi;
mod file;
pub mod file_system;
mod hash_key;
pub mod inode;
mod page_cache;
pub mod pipe;
mod procfs;
mod sysfs;
pub mod tmpfs;
use alloc::{string::String, sync::Arc};

use driver::BLOCK_DEVICE;
pub use fat32::FAT32FileSystem;
pub use fd_table::{Fd, FdInfo, FdTable};
pub use file::{File, FileMeta, SeekFrom};
pub use file_system::{FileSystem, FileSystemType, FILE_SYSTEM_MANAGER};
pub use hash_key::HashKey;
pub use inode::{Inode, InodeMode, InodeState};
use log::{debug, info, warn};
use memory::MapPermission;
pub use page_cache::PageCache;
use sync::mutex::SpinNoIrqLock;
pub use sysfs::{K_COVERAGE, K_COV_INODE};
use systype::{GeneralRet, SyscallErr};

use self::{ffi::StatFlags, file_system::FsDevice, inode::INODE_CACHE};
use crate::{fs::inode::FAST_PATH_CACHE, loader::get_app_data_by_name, stack_trace, utils::path};

type Mutex<T> = SpinNoIrqLock<T>;

fn create_mem_file(parent_inode: &Arc<dyn Inode>, name: &str) {
    stack_trace!();
    let inode = parent_inode
        .mknod_v(name, InodeMode::FileREG, None)
        .unwrap();
    let file = inode.open(inode.clone()).unwrap();
    file.sync_write(get_app_data_by_name(name).unwrap())
        .unwrap();
}

pub fn init() {
    stack_trace!();
    INODE_CACHE.init();

    // First we mount root fs
    #[cfg(feature = "tmpfs")]
    FILE_SYSTEM_MANAGER
        .mount(
            "/",
            // TODO: not sure
            "/dev/tmp",
            file_system::FsDevice::None,
            FileSystemType::TmpFS,
            StatFlags::ST_NOSUID,
        )
        .expect("rootfs init fail!");

    #[cfg(not(feature = "tmpfs"))]
    FILE_SYSTEM_MANAGER
        .mount(
            "/",
            "/dev/mmcblk0",
            file_system::FsDevice::BlockDevice(BLOCK_DEVICE.lock().as_ref().unwrap().clone()),
            FileSystemType::VFAT,
            StatFlags::ST_NOSUID,
        )
        .expect("rootfs init fail!");

    #[cfg(feature = "preliminary")]
    FILE_SYSTEM_MANAGER.mount(
        "/",
        "/dev/vda2",
        FsDevice::None,
        FileSystemType::VFAT,
        StatFlags::ST_NOSUID,
    );

    list_rootfs();

    let root_inode = FILE_SYSTEM_MANAGER.root_inode();

    root_inode.load_children();

    #[cfg(feature = "tmpfs")]
    let mem_apps = [
        "time-test",
        "busybox_testcode.sh",
        "busybox_cmd.txt",
        "busybox",
        "runtestcase",
        "shell",
        "lmbench_all",
        "lmbench_testcode.sh",
        "runtest.exe",
        "entry-static.exe",
        "run-static.sh",
    ];
    #[cfg(not(feature = "tmpfs"))]
    let mem_apps = ["busybox", "runtestcase", "shell"];
    for app in mem_apps {
        create_mem_file(&root_inode, app);
    }

    // For builtin commands
    let builtin_cmds = ["sleep", "ls"];
    for cmd in builtin_cmds {
        root_inode.mknod_v(cmd, InodeMode::FileREG, None).unwrap();
    }

    // Create some necessary dirs
    let dirs = ["dev", "proc", "tmp"];
    for dir in dirs {
        root_inode.mkdir_v(dir, InodeMode::FileDIR).unwrap();
    }

    let var_dir = root_inode.mkdir_v("var", InodeMode::FileDIR).unwrap();
    var_dir
        .mkdir_v("tmp", InodeMode::FileDIR)
        .expect("mkdir /var/tmp fail!");

    root_inode
        .mknod_v("lat_sig", InodeMode::FileREG, None)
        .unwrap();

    // let etc_dir = root_inode.mkdir_v("etc", InodeMode::FileDIR).unwrap();
    // let paths = ["ld-musl-riscv64-sf.path", "ld-musl-riscv64.path"];
    // for path in paths {
    //     let musl_dl_path = etc_dir.mknod_v(path, InodeMode::FileREG,
    // None).unwrap();     let file = musl_dl_path
    //         .open(musl_dl_path.clone(), OpenFlags::RDWR)
    //         .unwrap();
    //     file.sync_write("/\n/lib\n/lib64/lp64d\n/usr/lib\n".as_bytes())
    //         .unwrap();
    // }

    FILE_SYSTEM_MANAGER
        .mount(
            "/dev",
            "udev",
            FsDevice::None,
            FileSystemType::DevTmpFS,
            StatFlags::ST_NOSUID,
        )
        .expect("devfs init fail!");
    devfs::init();

    FILE_SYSTEM_MANAGER
        .mount(
            "/proc",
            "proc",
            FsDevice::None,
            FileSystemType::Proc,
            StatFlags::ST_NOSUID,
        )
        .expect("procfs init fail!");

    FILE_SYSTEM_MANAGER
        .mount(
            "/tmp",
            "tmp",
            FsDevice::None,
            FileSystemType::TmpFS,
            StatFlags::ST_NOSUID,
        )
        .expect("tmpfs init fail!");

    FILE_SYSTEM_MANAGER
        .mount(
            "/var/tmp",
            "var_tmp",
            FsDevice::None,
            FileSystemType::TmpFS,
            StatFlags::ST_NOSUID,
        )
        .expect("tmpfs init fail!");

    FAST_PATH_CACHE.init();

    sysfs::init();
    // list_rootfs();
}
pub const AT_FDCWD: isize = -100;

impl Default for OpenFlags {
    fn default() -> Self {
        Self::RDWR
    }
}

bitflags! {
    /// Open file flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OpenFlags: u32 {
        const APPEND = 1 << 10;
        const ASYNC = 1 << 13;
        const DIRECT = 1 << 14;
        const DSYNC = 1 << 12;
        const EXCL = 1 << 7;
        const NOATIME = 1 << 18;
        const NOCTTY = 1 << 8;
        const NOFOLLOW = 1 << 17;
        const PATH = 1 << 21;
        /// TODO: need to find 1 << 15
        const TEMP = 1 << 15;
        /// Read only
        const RDONLY = 0;
        /// Write only
        const WRONLY = 1 << 0;
        /// Read & Write
        const RDWR = 1 << 1;
        /// Allow create
        const CREATE = 1 << 6;
        /// Clear file and return an empty one
        const TRUNC = 1 << 9;
        /// Directory
        const DIRECTORY = 1 << 16;
        /// Enable the close-on-exec flag for the new file descriptor
        const CLOEXEC = 1 << 19;
        /// When possible, the file is opened in nonblocking mode
        const NONBLOCK = 1 << 11;
    }

    /// fcntl flags
    pub struct FcntlFlags: u32 {
        const FD_CLOEXEC = 1;
        const AT_EMPTY_PATH = 1 << 0;
        const AT_SYMLINK_NOFOLLOW = 1 << 8;
        const AT_EACCESS = 1 << 9;
        const AT_NO_AUTOMOUNT = 1 << 11;
        const AT_DUMMY = 1 << 12;
    }

    /// renameat flag
    pub struct Renameat2Flags: u32 {
        /// Go back to renameat
        const RENAME_NONE = 0;
        /// Atomically exchange oldpath and newpath.
        const RENAME_EXCHANGE = 1 << 1;
        /// Don't overwrite newpath of the rename. Return an error if newpath already exists.
        const RENAME_NOREPLACE = 1 << 0;
        /// This operation makes sense only for overlay/union filesystem implementations.
        const RENAME_WHITEOUT = 1 << 2;
    }

    /// faccessat flag
    pub struct FaccessatFlags: u32 {
        const F_OK = 0;
        const R_OK = 1 << 2;
        const W_OK = 1 << 1;
        const X_OK = 1 << 0;
    }
}

impl OpenFlags {
    pub fn readable(&self) -> bool {
        stack_trace!();
        self.contains(OpenFlags::RDONLY) || self.contains(OpenFlags::RDWR)
    }
    pub fn writable(&self) -> bool {
        stack_trace!();
        self.contains(OpenFlags::WRONLY) || self.contains(OpenFlags::RDWR)
    }
}

impl From<MapPermission> for OpenFlags {
    fn from(perm: MapPermission) -> Self {
        stack_trace!();
        let mut res = OpenFlags::from_bits(0).unwrap();
        if perm.contains(MapPermission::R) && perm.contains(MapPermission::W) {
            res |= OpenFlags::RDWR;
        } else if perm.contains(MapPermission::R) {
            res |= OpenFlags::RDONLY;
        } else if perm.contains(MapPermission::W) {
            res |= OpenFlags::WRONLY;
        }
        res
    }
}

#[allow(unused)]
pub fn print_dir_tree() {
    stack_trace!();
    info!("------------ dir tree: ------------");
    let parent = Arc::clone(&FILE_SYSTEM_MANAGER.root_inode());
    print_dir_recursively(parent, 1);
}

fn print_dir_recursively(inode: Arc<dyn Inode>, level: usize) {
    stack_trace!();
    let children = inode.metadata().inner.lock().children.clone();
    for child in children {
        for _ in 0..level {
            print!("-");
        }
        println!("{}", child.0);
        print_dir_recursively(child.1, level + 1);
    }
}

pub fn resolve_path_ffi(
    dirfd: isize,
    path: *const u8,
    flags: OpenFlags,
) -> GeneralRet<Arc<dyn Inode>> {
    stack_trace!();
    let (inode, path, parent) = path::path_to_inode_ffi(dirfd, path)?;
    _resolve_path(inode, parent, path, flags)
}

/// Resolve path at dirfd(except that `path` is absolute path)
pub fn resolve_path(dirfd: isize, path: &str, flags: OpenFlags) -> GeneralRet<Arc<dyn Inode>> {
    stack_trace!();
    let (inode, path, parent) = path::path_to_inode(dirfd, Some(path))?;
    _resolve_path(inode, parent, path, flags)
}

fn _resolve_path(
    inode: Option<Arc<dyn Inode>>,
    parent: Option<Arc<dyn Inode>>,
    path: String,
    flags: OpenFlags,
) -> GeneralRet<Arc<dyn Inode>> {
    stack_trace!();
    if inode.is_some() {
        return Ok(inode.unwrap());
    }
    if flags.contains(OpenFlags::CREATE) {
        let parent = match parent {
            Some(parent) => parent,
            None => {
                let parent_path = path::get_parent_dir(&path).unwrap();
                <dyn Inode>::lookup_from_root(&parent_path)
                    .ok()
                    .unwrap()
                    .0
                    .unwrap()
            }
        };
        let child_name = path::get_name(&path);
        debug!("create file {}", path);
        let res = {
            if flags.contains(OpenFlags::DIRECTORY) {
                parent.mkdir_v(child_name, InodeMode::FileDIR).unwrap()
            } else {
                // TODO dev id
                parent
                    .mknod_v(child_name, InodeMode::FileREG, None)
                    .unwrap()
            }
        };
        Ok(res)
    } else {
        warn!("parent dir {} doesn't exist", path);
        return Err(SyscallErr::ENOENT);
    }
}

fn list_rootfs() {
    stack_trace!();
    FILE_SYSTEM_MANAGER.root_inode().load_children();
    for sb in FILE_SYSTEM_MANAGER
        .root_inode()
        .metadata()
        .inner
        .lock()
        .children
        .iter()
    {
        println!("-- {}", sb.0);
    }
}