use alloc::{boxed::Box, sync::Arc};
use core::{
    mem::MaybeUninit,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    ptr,
};

use async_trait::async_trait;
use systype::{SysError, SysResult, SyscallResult};
use tcp::TcpSock;
use udp::UdpSock;
use unix::UnixSock;
use vfs_core::*;

use super::*;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
/// IPv4 address
pub struct SockAddrIn {
    pub family: u16,
    pub port: u16,
    pub addr: Ipv4Addr,
    pub zero: [u8; 8],
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
/// IPv6 address
pub struct SockAddrIn6 {
    pub family: u16,
    pub port: u16,
    pub flowinfo: u32,
    pub addr: Ipv6Addr,
    pub scope: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
/// Unix domain socket address
pub struct SockAddrUn {
    pub family: u16,
    pub path: [u8; 108],
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub enum SockAddr {
    SockAddrIn(SockAddrIn),
    SockAddrIn6(SockAddrIn6),
    SockAddrUn(SockAddrUn),
}

impl Into<SocketAddr> for SockAddr {
    fn into(self) -> SocketAddr {
        match self {
            SockAddr::SockAddrIn(v4) => SocketAddr::V4(SocketAddrV4::new(v4.addr, v4.port)),
            SockAddr::SockAddrIn6(v6) => {
                SocketAddr::V6(SocketAddrV6::new(v6.addr, v6.port, v6.flowinfo, v6.scope))
            }
            SockAddr::SockAddrUn(_) => {
                panic!("unix addr isn't Internet. You shouldn't convert to SocketAddr")
            }
        }
    }
}
#[async_trait]
pub trait ProtoOps: Sync + Send {
    fn bind(&self, _myaddr: SockAddr) -> SysResult<()> {
        Err(SysError::EOPNOTSUPP)
    }
    async fn connect(&self, _vaddr: SockAddr) -> SysResult<()> {
        Err(SysError::EOPNOTSUPP)
    }
}

/// linux中，socket面向用户空间，sock面向内核空间
pub struct Socket {
    /// socket类型
    pub types: SocketType,
    /// 套接字的核心，面向底层网络具体协议
    pub sk: Arc<dyn ProtoOps>,
    /// TODO:
    pub file: Arc<SocketFile>,
}

unsafe impl Sync for Socket {}
unsafe impl Send for Socket {}

impl Socket {
    pub fn new(domain: SocketAddressFamily, types: SocketType) -> Self {
        let mut nonblock = false;
        if types.contains(SocketType::NONBLOCK) {
            nonblock = true;
        }
        let sk: Arc<dyn ProtoOps> = match domain {
            SocketAddressFamily::AF_UNIX => Arc::new(UnixSock {}),
            SocketAddressFamily::AF_INET => {
                if types.contains(SocketType::STREAM) {
                    Arc::new(TcpSock::new(nonblock))
                } else if types.contains(SocketType::DGRAM) {
                    Arc::new(UdpSock::new(nonblock))
                } else {
                    unimplemented!()
                }
            }
            SocketAddressFamily::AF_INET6 => unimplemented!(),
        };

        Self {
            types,
            sk,
            file: unsafe { Arc::from_raw(ptr::null_mut()) },
        }
    }

    // pub fn bind(&self, addr: SockAddr) {
    //     self.sk.bind(myaddr)
    // }
}

/// sockfs是虚拟文件系统，所以在磁盘上不存在inode的表示，在内核中有struct
/// socket_alloc来表示内存中sockfs文件系统inode的相关结构体
// pub struct SocketAlloc {
//     socket: Socket,
//     meta: InodeMeta,
// }

// impl SocketAlloc {
//     pub fn new(types: SocketType) -> Self {
//         // TODO：add inode to sockfs
//         let meta = InodeMeta::new(InodeMode::SOCKET,
// Arc::<usize>::new_uninit(), 0);         let sk: Arc<dyn ProtoOps> = if
// types.contains(SocketType::STREAM) {             Arc::new(TcpSock {})
//         } else {
//             Arc::new(UdpSock {})
//         };
//         Self {
//             socket: Socket {
//                 types,
//                 sk,
//                 // TODO:
//                 file: unsafe { Arc::from_raw(ptr::null_mut()) },
//             },
//             meta,
//         }
//     }
// }

pub struct SocketFile {
    meta: FileMeta,
}

#[async_trait]
impl File for Socket {
    fn meta(&self) -> &FileMeta {
        &self.file.meta
    }

    async fn base_read_at(&self, _offset: usize, _buf: &mut [u8]) -> SyscallResult {
        // log::debug!("[TtyFile::base_read_at] buf len {}", buf.len());
        // let char_dev = &self
        //     .inode()
        //     .downcast_arc::<TtyInode>()
        //     .unwrap_or_else(|_| unreachable!())
        //     .char_dev;
        // let len = char_dev.read(buf).await;
        Ok(0)
    }

    async fn base_write_at(&self, _offset: usize, _buf: &[u8]) -> SyscallResult {
        // let utf8_buf: Vec<u8> = buf.iter().filter(|c| c.is_ascii()).map(|c|
        // *c).collect(); let char_dev = &self
        //     .inode()
        //     .downcast_arc::<TtyInode>()
        //     .unwrap_or_else(|_| unreachable!())
        //     .char_dev;
        // let len = char_dev.write(buf).await;
        Ok(0)
    }

    // async fn base_poll(&self, events: PollEvents) -> PollEvents {
    //     let mut res = PollEvents::empty();
    //     let char_dev = &self
    //         .inode()
    //         .downcast_arc::<TtyInode>()
    //         .unwrap_or_else(|_| unreachable!())
    //         .char_dev;
    //     if events.contains(PollEvents::IN) {
    //         if char_dev.poll_in().await {
    //             res |= PollEvents::IN;
    //         }
    //     }
    //     if events.contains(PollEvents::OUT) {
    //         if char_dev.poll_out().await {
    //             res |= PollEvents::OUT;
    //         }
    //     }
    //     log::debug!("[TtyFile::base_poll] ret events:{res:?}");
    //     res
    // }
}
