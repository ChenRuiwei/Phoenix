use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
};

use device_core::BlockDevice;
use systype::{SysError, SysResult};

use crate::{Dentry, MountFlags, Mutex, SuperBlock};

pub struct FileSystemTypeMeta {
    /// Name of this file system type.
    name: String,
    /// Super blocks.
    pub supers: Mutex<BTreeMap<String, Arc<dyn SuperBlock>>>,
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
    // NOTE: `self` cannot be `&Arc<Self>` for object safety
    // https://doc.rust-lang.org/reference/items/traits.html#object-safety
    fn base_mount(
        self: Arc<Self>,
        name: &str,
        parent: Option<Arc<dyn Dentry>>,
        flags: MountFlags,
        dev: Option<Arc<dyn BlockDevice>>,
    ) -> SysResult<Arc<dyn Dentry>>;

    /// Call when an instance of this filesystem should be shut down.
    fn kill_sb(&self, sb: Arc<dyn SuperBlock>) -> SysResult<()>;

    fn insert_sb(&self, abs_mount_path: &str, super_block: Arc<dyn SuperBlock>) {
        self.meta()
            .supers
            .lock()
            .insert(abs_mount_path.to_string(), super_block);
    }

    fn name(&self) -> &str {
        &self.meta().name
    }

    fn name_string(&self) -> String {
        self.meta().name.to_string()
    }
}

impl dyn FileSystemType {
    pub fn mount(
        self: &Arc<Self>,
        name: &str,
        parent: Option<Arc<dyn Dentry>>,
        flags: MountFlags,
        dev: Option<Arc<dyn BlockDevice>>,
    ) -> SysResult<Arc<dyn Dentry>> {
        self.clone().base_mount(name, parent, flags, dev)
    }

    pub fn get_sb(&self, abs_mount_path: &str) -> SysResult<Arc<dyn SuperBlock>> {
        self.meta()
            .supers
            .lock()
            .get(abs_mount_path)
            .cloned()
            .ok_or(SysError::ENOENT)
    }
}

bitflags::bitflags! {
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
        const RENAME_DOES_D_MOVE = 0x8000;
    }
}
