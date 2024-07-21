use alloc::{boxed::Box, sync::Arc};
use core::any::Any;

use async_trait::async_trait;
use downcast_rs::{impl_downcast, DowncastSync};
use log::warn;
use net::{poll_interfaces, NetPollState};
use spin::Mutex;
use systype::{SysError, SysResult, SyscallResult};
use tcp::TcpSock;
use udp::UdpSock;
use unix::UnixSock;
use vfs_core::*;

use super::*;

#[async_trait]
pub trait ProtoOps: Sync + Send + Any + DowncastSync {
    fn bind(&self, _myaddr: SockAddr) -> SysResult<()>;
    fn listen(&self) -> SysResult<()> {
        Err(SysError::EOPNOTSUPP)
    }
    async fn accept(&self) -> SysResult<Arc<dyn ProtoOps>> {
        Err(SysError::EOPNOTSUPP)
    }
    async fn connect(&self, _vaddr: SockAddr) -> SysResult<()> {
        Err(SysError::EOPNOTSUPP)
    }
    fn peer_addr(&self) -> SysResult<SockAddr> {
        Err(SysError::EOPNOTSUPP)
    }
    fn local_addr(&self) -> SysResult<SockAddr> {
        Err(SysError::EOPNOTSUPP)
    }
    async fn sendto(&self, _buf: &[u8], _vaddr: Option<SockAddr>) -> SysResult<usize> {
        Err(SysError::EOPNOTSUPP)
    }
    async fn recvfrom(&self, _buf: &mut [u8]) -> SysResult<(usize, SockAddr)> {
        Err(SysError::EOPNOTSUPP)
    }
    async fn poll(&self) -> NetPollState {
        log::error!("[net poll] unimplemented");
        NetPollState {
            readable: false,
            writable: false,
        }
    }
    fn shutdown(&self, how: SocketShutdownFlag) -> SysResult<()> {
        Err(SysError::EOPNOTSUPP)
    }
}

// Todo: Maybe it needn't
impl_downcast!(sync ProtoOps);
/// linux中，socket面向用户空间，sock面向内核空间
pub struct Socket {
    /// socket类型
    pub types: SocketType,
    /// 套接字的核心，面向底层网络具体协议
    pub sk: Arc<dyn ProtoOps>,
    /// TODO:
    pub meta: FileMeta,
}

unsafe impl Sync for Socket {}
unsafe impl Send for Socket {}

impl Socket {
    pub fn new(domain: SaFamily, types: SocketType, nonblock: bool) -> Self {
        let sk: Arc<dyn ProtoOps> = match domain {
            SaFamily::AF_UNIX => Arc::new(UnixSock {}),
            SaFamily::AF_INET | SaFamily::AF_INET6 => match types {
                SocketType::STREAM => Arc::new(TcpSock::new(nonblock)),
                SocketType::DGRAM => Arc::new(UdpSock::new(nonblock)),
                _ => unimplemented!(),
            },
        };
        let flags = if nonblock {
            OpenFlags::O_RDWR | OpenFlags::O_NONBLOCK
        } else {
            OpenFlags::O_RDWR
        };
        Self {
            types,
            sk,
            meta: FileMeta {
                dentry: Arc::<usize>::new_zeroed(),
                inode: Arc::<usize>::new_zeroed(),
                pos: 0.into(),
                flags: Mutex::new(flags),
            },
        }
    }

    pub fn from_another(another: &Self, sk: Arc<dyn ProtoOps>) -> Self {
        Self {
            types: another.types,
            sk,
            meta: FileMeta {
                dentry: Arc::<usize>::new_zeroed(),
                inode: Arc::<usize>::new_zeroed(),
                pos: 0.into(),
                flags: Mutex::new(OpenFlags::O_RDWR),
            },
        }
    }
}

#[async_trait]
impl File for Socket {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, _offset: usize, buf: &mut [u8]) -> SyscallResult {
        if buf.len() == 0 {
            return Ok(0);
        }
        // TODO: should add this?
        poll_interfaces();
        let bytes = self.sk.recvfrom(buf).await.map(|e| e.0)?;
        warn!("[socket read] expect: {:?} exact: {bytes}", buf.len());
        Ok(bytes)
    }

    async fn base_write_at(&self, _offset: usize, buf: &[u8]) -> SyscallResult {
        if buf.len() == 0 {
            return Ok(0);
        }
        // TODO: should add this?
        poll_interfaces();
        let bytes = self.sk.sendto(buf, None).await?;
        warn!("[socket write] expect: {:?} exact: {bytes}", buf.len());
        Ok(bytes)
    }

    async fn base_poll(&self, events: PollEvents) -> PollEvents {
        let mut res = PollEvents::empty();
        poll_interfaces();
        let netstate = self.sk.poll().await;
        if events.contains(PollEvents::IN) {
            if netstate.readable {
                res |= PollEvents::IN;
            }
        }
        if events.contains(PollEvents::OUT) {
            if netstate.writable {
                res |= PollEvents::OUT;
            }
        }
        log::info!("[Socket::base_poll] ret events:{res:?} {netstate:?}");
        res
    }
}

/// sockfs是虚拟文件系统，所以在磁盘上不存在inode的表示，在内核中有struct
/// socket_alloc来表示内存中sockfs文件系统inode的相关结构体
#[allow(dead_code)]
pub struct SocketAlloc {
    socket: Socket,
    meta: InodeMeta,
}

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
