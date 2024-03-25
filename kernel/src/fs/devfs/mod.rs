use alloc::{string::ToString, sync::Arc, vec::Vec};
use core::sync::atomic::AtomicUsize;

use log::debug;
use systype::{GeneralRet, SyscallErr};

use self::{
    cpu_dma_latency::LatencyInode, null::NullInode, rtc::RtcInode, tty::TtyInode,
    urandom::UrandomInode, zero::ZeroInode,
};
use super::{
    file_system::{FileSystem, FileSystemMeta},
    inode::{InodeMeta, InodeMode},
    tmpfs::inode::TmpInode,
    FileSystemType, Inode,
};
use crate::{
    fs::{devfs::r#loop::LoopInode, ffi::StatFlags, hash_key::HashKey, inode::INODE_CACHE},
    stack_trace,
    utils::path,
};

mod block_device;
mod cpu_dma_latency;
pub mod r#loop;
mod null;
mod rtc;
mod tty;
mod urandom;
mod zero;

pub use tty::TTY;

/// i.e. /dev
pub struct DevRootInode {
    metadata: Option<InodeMeta>,
}

impl Inode for DevRootInode {
    fn mknod(
        &self,
        this: Arc<dyn Inode>,
        name: &str,
        _mode: InodeMode,
        dev_id: Option<usize>,
    ) -> GeneralRet<Arc<dyn Inode>> {
        stack_trace!();
        debug!("[DevRootInode::mknod]: mknod: {}", name);
        let dev_id = dev_id.unwrap();
        let inode = if dev_id < DEV_NAMES.len() {
            let creator = DEV_NAMES[dev_id].2;
            creator(this.clone(), DEV_NAMES[dev_id].0)
        } else {
            // loop device
            let path = this.metadata().path.clone() + "/" + name;
            Arc::new(LoopInode::new(this.clone(), &path, dev_id))
        };
        this.metadata()
            .inner
            .lock()
            .children
            .insert(inode.metadata().name.clone(), inode.clone());
        Ok(inode)
    }

    fn set_metadata(&mut self, meta: InodeMeta) {
        stack_trace!();
        self.metadata = Some(meta);
    }

    fn metadata(&self) -> &InodeMeta {
        stack_trace!();
        &self.metadata.as_ref().unwrap()
    }

    /// Load children like 'sda' 'null' etc
    fn load_children_from_disk(&self, _this: Arc<dyn Inode>) {
        stack_trace!();
        debug!("[DevRootInode::load_children_from_disk]: there is nothing we should do.");
    }

    /// Delete inode in disk
    fn delete_child(&self, _child_name: &str) {
        stack_trace!();
        todo!()
    }
    fn child_removeable(&self) -> GeneralRet<()> {
        stack_trace!();
        Err(SyscallErr::EPERM)
    }
}

impl DevRootInode {
    pub fn new() -> Self {
        stack_trace!();
        Self { metadata: None }
    }
}

/// TODO: resolve dtb instead of constant list
const DEV_NAMES: [(
    &str,
    InodeMode,
    fn(parent: Arc<dyn Inode>, path: &str) -> Arc<dyn Inode>,
); 7] = [
    ("/dev/vda2", InodeMode::FileBLK, |parent, path| {
        Arc::new(TmpInode::new(
            Some(parent),
            path::get_name(path),
            InodeMode::FileDIR,
        ))
    }),
    ("/dev/zero", InodeMode::FileCHR, |parent, path| {
        Arc::new(ZeroInode::new(parent, path))
    }),
    ("/dev/null", InodeMode::FileCHR, |parent, path| {
        Arc::new(NullInode::new(parent, path))
    }),
    ("/dev/tty", InodeMode::FileCHR, |parent, path| {
        Arc::new(TtyInode::new(parent, path))
    }),
    ("/dev/rtc", InodeMode::FileCHR, |parent, path| {
        Arc::new(RtcInode::new(parent, path))
    }),
    ("/dev/urandom", InodeMode::FileCHR, |parent, path| {
        Arc::new(UrandomInode::new(parent, path))
    }),
    (
        "/dev/cpu_dma_latency",
        InodeMode::FileCHR,
        |parent, path| Arc::new(LatencyInode::new(parent, path)),
    ),
];

pub struct DevFs {
    metadata: FileSystemMeta,
    // _id_allocator: AtomicUsize,
    // dev_mgr: Arc<DevManager>,
}

impl DevFs {
    pub fn new(
        mount_point: &str,
        dev_name: &str,
        fstype: FileSystemType,
        flags: StatFlags,
        fa_inode: Option<Arc<dyn Inode>>,
        covered_inode: Option<Arc<dyn Inode>>,
        covered_fs: Option<Arc<dyn FileSystem>>,
    ) -> GeneralRet<Self> {
        stack_trace!();
        let mut raw_root_inode = DevRootInode::new();
        raw_root_inode.root_init(Option::clone(&fa_inode), mount_point, InodeMode::FileDIR, 0)?;
        let root_inode = Arc::new(raw_root_inode);

        let id_allocator = AtomicUsize::new(0);

        let parent_ino = root_inode.metadata().ino;
        for (dev_name2, inode_mode, _) in DEV_NAMES {
            let child = root_inode.mknod(
                root_inode.clone(),
                dev_name2,
                inode_mode,
                Some(id_allocator.fetch_add(1, core::sync::atomic::Ordering::AcqRel)),
            )?;
            let child_name = child.metadata().name.clone();
            let key = HashKey::new(parent_ino, child_name);
            INODE_CACHE.insert(key, child.clone());
            debug!("insert {} finished", dev_name2);
        }

        Ok(Self {
            metadata: FileSystemMeta {
                dev_name: dev_name.to_string(),
                mount_point: mount_point.to_string(),
                fstype,
                flags,
                root_inode,
                fa_inode,
                covered_inode,
                covered_fs,
                s_dirty: Vec::new(),
            },
            // id_allocator: id_allocator,
        })
    }
}

impl FileSystem for DevFs {
    fn metadata(&self) -> &FileSystemMeta {
        stack_trace!();
        &self.metadata
    }

    // fn sync_fs(&self) {}
}

pub fn init() {
    stack_trace!();
    tty::init();
}