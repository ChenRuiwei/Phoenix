use alloc::{collections::BTreeMap, sync::Arc};

use vfs::{FileSystemType, FileSystemTypeMeta, SuperBlockMeta};

use crate::Mutex;

pub struct FatFsType {
    meta: FileSystemTypeMeta,
}

impl FileSystemType for FatFsType {
    fn meta(&self) -> &FileSystemTypeMeta {
        &self.meta
    }

    fn set_meta(&self, meta: FileSystemTypeMeta) {
        self.meta = meta
    }

    fn mount(
        self: &Self,
        abs_mount_path: &str,
        flags: vfs::MountFlags,
        dev: Option<Arc<dyn driver::BlockDevice>>,
    ) -> systype::SysResult<Arc<dyn vfs::SuperBlock>> {
        todo!()
    }

    fn kill_sb(&self, sb: Arc<dyn vfs::SuperBlock>) -> systype::SysResult<()> {
        todo!()
    }
}

pub struct FatFsSuperBlock {
    meta: SuperBlockMeta,
}
