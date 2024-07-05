use alloc::sync::{Arc, Weak};
use core::mem::MaybeUninit;

use device_core::DevId;
use downcast_rs::{impl_downcast, DowncastSync};
use page::PageCache;
use systype::{SysResult, SyscallResult};
use time::timespec::TimeSpec;

use crate::{alloc_ino, Mutex, Stat, SuperBlock};

pub struct InodeMeta {
    /// Inode number.
    pub ino: usize,
    /// Mode of inode.
    pub mode: InodeMode,
    pub dev_id: Option<DevId>,
    pub super_block: Weak<dyn SuperBlock>,

    pub page_cache: Option<PageCache>,
    pub inner: Mutex<InodeMetaInner>,
}

pub struct InodeMetaInner {
    /// Size of a file in bytes.
    pub size: usize,
    /// Last access time.
    pub atime: TimeSpec,
    /// Last modification time.
    pub mtime: TimeSpec,
    /// Last status change time.
    pub ctime: TimeSpec,
    ///
    pub state: InodeState,
}

impl Drop for InodeMeta {
    fn drop(&mut self) {
        match self.inner.lock().state {
            InodeState::UnInit => {}
            InodeState::Sync => {}
            InodeState::Dirty => {
                self.page_cache.as_ref().map(|page_cache| {
                    log::warn!("[InodeMeta::drop] flush page cache to disk");
                    page_cache.flush()
                });
            }
            InodeState::Removed => {}
        }
    }
}

impl InodeMeta {
    pub fn new(mode: InodeMode, super_block: Arc<dyn SuperBlock>, size: usize) -> Self {
        let itype = mode.to_type();
        let address_space = if (itype.is_file() || itype.is_block_device())
            && (super_block.meta().device.is_some())
        {
            Some(PageCache::new())
        } else {
            None
        };
        Self {
            ino: alloc_ino(),
            mode,
            super_block: Arc::downgrade(&super_block),
            dev_id: None,
            page_cache: address_space,
            inner: Mutex::new(InodeMetaInner {
                size,
                atime: TimeSpec::default(),
                mtime: TimeSpec::default(),
                ctime: TimeSpec::default(),
                state: InodeState::UnInit,
            }),
        }
    }
}

pub trait Inode: Send + Sync + DowncastSync {
    fn meta(&self) -> &InodeMeta;

    fn get_attr(&self) -> SysResult<Stat>;

    fn base_truncate(&self, len: usize) -> SysResult<()> {
        todo!()
    }

    /// Calculates the block index on the underlying block device for a given
    /// file offset.
    fn base_get_blk_idx(&self, offset: usize) -> SysResult<usize> {
        todo!()
    }
}

impl dyn Inode {
    pub fn ino(&self) -> usize {
        self.meta().ino
    }

    pub fn dev_id(&self) -> DevId {
        self.meta().dev_id.expect("should own a dev id")
    }

    pub fn itype(&self) -> InodeType {
        self.meta().mode.to_type()
    }

    pub fn page_cache<'a>(self: &'a Arc<dyn Inode>) -> Option<&'a PageCache> {
        self.meta().page_cache.as_ref()
    }

    pub fn size(&self) -> usize {
        self.meta().inner.lock().size
    }

    pub fn set_size(&self, size: usize) {
        self.meta().inner.lock().size = size;
    }

    pub fn state(&self) -> InodeState {
        self.meta().inner.lock().state
    }

    pub fn set_state(&self, state: InodeState) {
        self.meta().inner.lock().state = state;
    }

    pub fn truncate(&self, len: usize) -> SyscallResult {
        log::info!(
            "[Inode::truncate] len:{len:#x}, origin size:{:#x}",
            self.size()
        );
        // if self.size() < len {
        //     self.address_space()
        // }
        self.base_truncate(len).map(|_| 0)
    }

    pub fn get_blk_idx(&self, offset: usize) -> SysResult<usize> {
        self.base_get_blk_idx(offset)
    }

    pub fn super_block(&self) -> Arc<dyn SuperBlock> {
        self.meta().super_block.upgrade().unwrap()
    }
}

impl_downcast!(sync Inode);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InodeState {
    /// Init state, indicates that this inode is not loaded from disk yet.
    UnInit,
    /// already sync
    Sync,
    Dirty,
    Removed,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    pub struct InodeMode: u32 {
        /// Type.
        const TYPE_MASK = 0o170000;
        /// FIFO.
        const FIFO  = 0o010000;
        /// Character device.
        const CHAR  = 0o020000;
        /// Directory
        const DIR   = 0o040000;
        /// Block device
        const BLOCK = 0o060000;
        /// Regular file.
        const FILE  = 0o100000;
        /// Symbolic link.
        const LINK  = 0o120000;
        /// Socket
        const SOCKET = 0o140000;

        /// Set-user-ID on execution.
        const SET_UID = 0o4000;
        /// Set-group-ID on execution.
        const SET_GID = 0o2000;
        /// sticky bit
        const STICKY = 0o1000;
        /// Read, write, execute/search by owner.
        const OWNER_MASK = 0o700;
        /// Read permission, owner.
        const OWNER_READ = 0o400;
        /// Write permission, owner.
        const OWNER_WRITE = 0o200;
        /// Execute/search permission, owner.
        const OWNER_EXEC = 0o100;

        /// Read, write, execute/search by group.
        const GROUP_MASK = 0o70;
        /// Read permission, group.
        const GROUP_READ = 0o40;
        /// Write permission, group.
        const GROUP_WRITE = 0o20;
        /// Execute/search permission, group.
        const GROUP_EXEC = 0o10;

        /// Read, write, execute/search by others.
        const OTHER_MASK = 0o7;
        /// Read permission, others.
        const OTHER_READ = 0o4;
        /// Write permission, others.
        const OTHER_WRITE = 0o2;
        /// Execute/search permission, others.
        const OTHER_EXEC = 0o1;
    }
}

impl InodeMode {
    pub fn to_type(&self) -> InodeType {
        (*self).into()
    }

    pub fn from_type(itype: InodeType) -> Self {
        let perm_mode = InodeMode::OWNER_READ | InodeMode::OWNER_WRITE | InodeMode::OTHER_EXEC;
        let file_mode = match itype {
            InodeType::Dir => InodeMode::DIR,
            InodeType::File => InodeMode::FILE,
            InodeType::SymLink => InodeMode::LINK,
            InodeType::CharDevice => InodeMode::CHAR,
            InodeType::BlockDevice => InodeMode::BLOCK,
            InodeType::Fifo => InodeMode::FIFO,
            InodeType::Socket => InodeMode::SOCKET,
            InodeType::Unknown => InodeMode::TYPE_MASK,
        };
        file_mode | perm_mode
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum InodeType {
    Unknown = 0,
    Fifo = 0o1,
    CharDevice = 0o2,
    Dir = 0o4,
    BlockDevice = 0o6,
    File = 0o10,
    SymLink = 0o12,
    Socket = 0o14,
}

impl From<u8> for InodeType {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Unknown,
            0o1 => Self::Fifo,
            0o2 => Self::CharDevice,
            0o4 => Self::Dir,
            0o6 => Self::BlockDevice,
            0o10 => Self::File,
            0o12 => Self::SymLink,
            0o14 => Self::Socket,
            _ => Self::Unknown,
        }
    }
}

impl From<char> for InodeType {
    fn from(value: char) -> Self {
        match value {
            '-' => Self::File,
            'd' => Self::Dir,
            'l' => Self::SymLink,
            'c' => Self::CharDevice,
            'b' => Self::BlockDevice,
            'p' => Self::Fifo,
            's' => Self::Socket,
            _ => Self::Unknown,
        }
    }
}

impl From<InodeMode> for InodeType {
    fn from(mode: InodeMode) -> Self {
        match mode.intersection(InodeMode::TYPE_MASK) {
            InodeMode::DIR => InodeType::Dir,
            InodeMode::FILE => InodeType::File,
            InodeMode::LINK => InodeType::SymLink,
            InodeMode::CHAR => InodeType::CharDevice,
            InodeMode::BLOCK => InodeType::BlockDevice,
            InodeMode::FIFO => InodeType::Fifo,
            InodeMode::SOCKET => InodeType::Socket,
            _ => InodeType::Unknown,
        }
    }
}

impl InodeType {
    /// Tests whether this node type represents a regular file.
    pub const fn is_file(self) -> bool {
        matches!(self, Self::File)
    }

    /// Tests whether this node type represents a directory.
    pub const fn is_dir(self) -> bool {
        matches!(self, Self::Dir)
    }

    /// Tests whether this node type represents a symbolic link.
    pub const fn is_symlink(self) -> bool {
        matches!(self, Self::SymLink)
    }

    /// Returns `true` if this node type is a block device.
    pub const fn is_block_device(self) -> bool {
        matches!(self, Self::BlockDevice)
    }

    /// Returns `true` if this node type is a char device.
    pub const fn is_char_device(self) -> bool {
        matches!(self, Self::CharDevice)
    }

    /// Returns `true` if this node type is a fifo.
    pub const fn is_fifo(self) -> bool {
        matches!(self, Self::Fifo)
    }

    /// Returns `true` if this node type is a socket.
    pub const fn is_socket(self) -> bool {
        matches!(self, Self::Socket)
    }

    /// Returns a character representation of the node type.
    ///
    /// For example, `d` for directory, `-` for regular file, etc.
    pub const fn as_char(self) -> char {
        match self {
            Self::Fifo => 'p',
            Self::CharDevice => 'c',
            Self::Dir => 'd',
            Self::BlockDevice => 'b',
            Self::File => '-',
            Self::SymLink => 'l',
            Self::Socket => 's',
            _ => '?',
        }
    }
}

impl<T: Send + Sync + 'static> Inode for MaybeUninit<T> {
    fn meta(&self) -> &InodeMeta {
        todo!()
    }

    fn get_attr(&self) -> SysResult<Stat> {
        todo!()
    }
}
