use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
};

use driver::BlockDevice;
use systype::SysResult;

use crate::{File, MountFlags, Mutex, SuperBlock};

pub struct FileSystemTypeMeta {
    /// Name of this file system type.
    name: String,
    /// Super blocks.
    supers: Mutex<BTreeMap<String, Arc<dyn SuperBlock>>>,
}

impl FileSystemTypeMeta {
    pub fn new(name: &str) -> FileSystemTypeMeta {
        Self {
            name: name.to_string(),
            supers: Mutex::new(BTreeMap::new()),
        }
    }
}

pub trait FileSystemType: Send + Sync {
    fn meta(&self) -> &FileSystemTypeMeta;

    /// Call when a new instance of this filesystem should be mounted.
    // NOTE: `&Arc<Self>` in a trait that `Self` has to be sized
    // https://stackoverflow.com/questions/70814508/understanding-the-trait-x-cannot-be-made-into-an-object-for-mut-boxself-p
    fn mount(
        self: &Arc<Self>,
        abs_mount_path: &str,
        flags: MountFlags,
        dev: Option<Arc<dyn BlockDevice>>,
    ) -> SysResult<Arc<dyn SuperBlock>>
    where
        Self: Sized;

    /// Call when an instance of this filesystem should be shut down.
    fn kill_sb(&self, sb: Arc<dyn SuperBlock>) -> SysResult<()>;
}

impl dyn FileSystemType {
    fn fs_name(&self) -> String {
        self.meta().name.clone()
    }

    fn insert(&self, abs_mount_path: String, super_block: Arc<dyn SuperBlock>) {
        self.meta()
            .supers
            .lock()
            .insert(abs_mount_path, super_block);
    }
}

bitflags! {
    pub struct FileSystemFlags:u32{
         /// The file system requires a device.
         const REQUIRES_DEV = 0x1;
        /// The options provided when mounting are in binary form.
        const BINARY_MOUNTDATA = 0x2;
        /// The file system has a subtype. It is extracted from the name and passed in as a parameter.
        const HAS_SUBTYPE = 0x4;
         /// The file system can be mounted by userns root.
        const USERNS_MOUNT = 0x8;
        /// Disables fanotify permission events.
        const DISALLOW_NOTIFY_PERM = 0x10;
        /// The file system has been updated to handle vfs idmappings.
        const ALLOW_IDMAP = 0x20;
        /// FS uses multigrain timestamps
        const MGTIME = 0x40;
        /// The file systen will handle `d_move` during `rename` internally.
        const RENAME_DOES_D_MOVE = 0x8000; //32768
    }
}
