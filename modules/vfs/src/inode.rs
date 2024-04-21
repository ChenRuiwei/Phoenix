use alloc::{string::String, sync::Arc, vec::Vec};

use systype::{SysError, SysResult};

use crate::{
    file::File,
    utils::{FileStat, NodePermission, RenameFlag, Time, TimeSpec},
    OpenFlags,
};

pub struct InodeAttr {
    /// File mode.
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    /// File size, in bytes.
    ///
    /// For truncate
    pub size: u64,
    pub atime: TimeSpec, // 最后访问时间
    pub mtime: TimeSpec, // 最后修改时间
    pub ctime: TimeSpec, // 最后改变时间
}

pub struct InodeMeta {
    /// Inode number.
    pub ino: usize,
    /// Type of inode.
    pub inode_type: InodeType,
}

pub trait Inode {
    fn open(&self, this: Arc<dyn Inode>) -> SysResult<Arc<dyn File>>;

    fn lookup(&self, _name: &str) -> SysResult<Arc<dyn Inode>>;

    fn node_type(&self) -> InodeType;
}

// 文件与文件夹类型
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum InodeType {
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
