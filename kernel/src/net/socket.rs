use alloc::{boxed::Box, sync::Arc};
use core::any::Any;

use async_trait::async_trait;
use downcast_rs::{impl_downcast, DowncastSync};
use log::warn;
use net::{poll_interfaces, tcp::TcpSocket, udp::UdpSocket, IpEndpoint, NetPollState};
use spin::Mutex;
use systype::{SysError, SysResult, SyscallResult};
use unix::UnixSocket;
use vfs_core::*;

use super::*;
use crate::processor::hart::current_task;

pub enum Sock {
    Tcp(TcpSocket),
    Udp(UdpSocket),
    Unix(UnixSocket),
}

impl Sock {
    pub fn set_nonblocking(&self) {
        match self {
            Sock::Tcp(tcp) => tcp.set_nonblocking(true),
            Sock::Udp(udp) => udp.set_nonblocking(true),
            Sock::Unix(_) => unimplemented!(),
        }
    }

    pub fn bind(&self, local_addr: IpEndpoint) -> SysResult<()> {
        match self {
            Sock::Tcp(tcp) => tcp.bind(local_addr),
            Sock::Udp(udp) => udp.bind(local_addr),
            Sock::Unix(_) => unimplemented!(),
        }
    }

    pub fn listen(&self) -> SysResult<()> {
        match self {
            Sock::Tcp(tcp) => tcp.listen(current_task().waker_ref().as_ref().unwrap()),
            Sock::Udp(udp) => Err(SysError::EOPNOTSUPP),
            Sock::Unix(_) => unimplemented!(),
        }
    }

    pub async fn accept(&self) -> SysResult<TcpSocket> {
        match self {
            Sock::Tcp(tcp) => {
                let new_tcp = tcp.accept().await?;
                Ok(new_tcp)
            }
            Sock::Udp(udp) => Err(SysError::EOPNOTSUPP),
            Sock::Unix(_) => unimplemented!(),
        }
    }

    pub async fn connect(&self, remote_addr: IpEndpoint) -> SysResult<()> {
        match self {
            Sock::Tcp(tcp) => tcp.connect(remote_addr).await,
            Sock::Udp(udp) => udp.connect(remote_addr),
            Sock::Unix(_) => unimplemented!(),
        }
    }

    pub fn peer_addr(&self) -> SysResult<IpEndpoint> {
        match self {
            Sock::Tcp(tcp) => tcp.peer_addr(),
            Sock::Udp(udp) => udp.peer_addr(),
            Sock::Unix(_) => unimplemented!(),
        }
    }

    pub fn local_addr(&self) -> SysResult<IpEndpoint> {
        match self {
            Sock::Tcp(tcp) => tcp.local_addr(),
            Sock::Udp(udp) => udp.local_addr(),
            Sock::Unix(_) => unimplemented!(),
        }
    }
    pub async fn sendto(&self, buf: &[u8], remote_addr: Option<IpEndpoint>) -> SysResult<usize> {
        match self {
            Sock::Tcp(tcp) => tcp.send(buf).await,
            Sock::Udp(udp) => match remote_addr {
                Some(addr) => udp.send_to(buf, addr).await,
                None => udp.send(buf).await,
            },
            Sock::Unix(_) => unimplemented!(),
        }
    }
    pub async fn recvfrom(&self, buf: &mut [u8]) -> SysResult<(usize, IpEndpoint)> {
        match self {
            Sock::Tcp(tcp) => {
                let bytes = tcp.recv(buf).await?;
                Ok((bytes, tcp.peer_addr()?))
            }
            Sock::Udp(udp) => udp.recv_from(buf).await,
            Sock::Unix(_) => unimplemented!(),
        }
    }
    pub async fn poll(&self) -> NetPollState {
        match self {
            Sock::Tcp(tcp) => tcp.poll().await,
            Sock::Udp(udp) => udp.poll().await,
            Sock::Unix(_) => unimplemented!(),
        }
    }

    pub fn shutdown(&self, how: u8) -> SysResult<()> {
        match self {
            Sock::Tcp(tcp) => tcp.shutdown(how),
            Sock::Udp(udp) => udp.shutdown(),
            Sock::Unix(_) => unimplemented!(),
        }
    }
}

/// linux中，socket面向用户空间，sock面向内核空间
pub struct Socket {
    /// socket类型
    pub types: SocketType,
    /// 套接字的核心，面向底层网络具体协议
    pub sk: Sock,
    /// TODO:
    pub meta: FileMeta,
}

unsafe impl Sync for Socket {}
unsafe impl Send for Socket {}

impl Socket {
    pub fn new(domain: SaFamily, types: SocketType, nonblock: bool) -> Self {
        let sk = match domain {
            SaFamily::AF_UNIX => Sock::Unix(UnixSocket {}),
            SaFamily::AF_INET | SaFamily::AF_INET6 => match types {
                SocketType::STREAM => Sock::Tcp(TcpSocket::new()),
                SocketType::DGRAM => Sock::Udp(UdpSocket::new()),
                _ => unimplemented!(),
            },
        };
        let flags = if nonblock {
            sk.set_nonblocking();
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

    pub fn from_another(another: &Self, sk: Sock) -> Self {
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
        if netstate.hangup {
            log::warn!("[Socket::bask_poll] PollEvents is hangup");
            res |= PollEvents::HUP;
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
