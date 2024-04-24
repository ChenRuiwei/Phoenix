use alloc::{
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::sync::atomic::AtomicUsize;

use downcast_rs::{impl_downcast, DowncastSync};
use spin::Mutex;
use systype::{SysError, SysResult};

use crate::{
    alloc_ino,
    file::File,
    super_block,
    utils::{NodePermission, RenameFlag, Stat, Time, TimeSpec},
    Dentry, SuperBlock,
};

pub struct InodeMeta {
    /// Inode number.
    pub ino: usize,
    /// mode of inode.
    pub mode: InodeMode,
    pub super_block: Weak<dyn SuperBlock>,

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
}

impl InodeMeta {
    pub fn new(mode: InodeMode, super_block: Arc<dyn SuperBlock>, size: usize) -> Self {
        Self {
            ino: alloc_ino(),
            mode,
            super_block: Arc::downgrade(&super_block),
            inner: Mutex::new(InodeMetaInner {
                size,
                atime: TimeSpec::default(),
                mtime: TimeSpec::default(),
                ctime: TimeSpec::default(),
            }),
        }
    }
}

pub trait Inode: Send + Sync + DowncastSync {
    fn meta(&self) -> &InodeMeta;

    /// Called by the open(2) and creat(2) system calls. Create a inode for a
    /// dentry in the directory inode.
    fn create(&self, dentry: Arc<dyn Dentry>, mode: InodeMode) -> SysResult<()>;

    fn get_attr(&self) -> SysResult<Stat>;
}

impl dyn Inode {
    pub fn mode(&self) -> InodeMode {
        self.meta().mode
    }

    pub fn size(&self) -> usize {
        self.meta().inner.lock().size
    }

    pub fn set_size(&self, size: usize) {
        self.meta().inner.lock().size = size;
    }
}

impl_downcast!(sync Inode);

// 文件与文件夹类型
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum InodeMode {
    // 未知类型
    Unknown = 0,
    // 先进先出类型（如管道）
    Fifo = 0o1,
    // 字符设备
    CharDevice = 0o2,
    // 文件夹
    Dir = 0o4,
    // 块设备
    BlockDevice = 0o6,
    // 普通文件
    File = 0o10,
    // 符号链接
    SymLink = 0o12,
    // 套接字
    Socket = 0o14,
}

impl From<u8> for InodeMode {
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

impl From<char> for InodeMode {
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

impl InodeMode {
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
