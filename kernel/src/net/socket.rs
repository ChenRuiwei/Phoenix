use alloc::{boxed::Box, sync::Arc};

use addr::SockAddr;
use async_trait::async_trait;
use log::warn;
use net::{
    addr::{UNSPECIFIED_ENDPOINT_V4, UNSPECIFIED_IPV4},
    poll_interfaces,
    tcp::TcpSocket,
    udp::UdpSocket,
    IpEndpoint, IpListenEndpoint, NetPollState,
};
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

    pub fn bind(&self, sockfd: usize, local_addr: SockAddr) -> SysResult<()> {
        match self {
            Sock::Tcp(tcp) => {
                // HACK
                let local_addr = local_addr.into_listen_endpoint();
                let addr = if local_addr.addr.is_none() {
                    UNSPECIFIED_IPV4
                } else {
                    local_addr.addr.unwrap()
                };
                tcp.bind(IpEndpoint::new(addr, local_addr.port))
            }
            Sock::Udp(udp) => {
                let local_addr = local_addr.into_listen_endpoint();
                if let Some(prev_fd) = udp.check_bind(sockfd, local_addr) {
                    current_task()
                        .with_mut_fd_table(|table| table.dup3_with_flags(prev_fd, sockfd))?;
                    return Ok(());
                }
                udp.bind(local_addr)
            }
            Sock::Unix(_) => unimplemented!(),
        }
    }

    pub fn listen(&self) -> SysResult<()> {
        match self {
            Sock::Tcp(tcp) => tcp.listen(current_task().waker_ref().as_ref().unwrap()),
            Sock::Udp(_udp) => Err(SysError::EOPNOTSUPP),
            Sock::Unix(_) => unimplemented!(),
        }
    }

    pub async fn accept(&self) -> SysResult<TcpSocket> {
        match self {
            Sock::Tcp(tcp) => {
                let new_tcp = tcp.accept().await?;
                Ok(new_tcp)
            }
            Sock::Udp(_udp) => Err(SysError::EOPNOTSUPP),
            Sock::Unix(_) => unimplemented!(),
        }
    }

    pub async fn connect(&self, remote_addr: SockAddr) -> SysResult<()> {
        match self {
            Sock::Tcp(tcp) => {
                let remote_addr = remote_addr.into_endpoint();
                tcp.connect(remote_addr).await
            }
            Sock::Udp(udp) => {
                let remote_addr = remote_addr.into_endpoint();
                udp.connect(remote_addr)
            }
            Sock::Unix(_) => unimplemented!(),
        }
    }

    pub fn peer_addr(&self) -> SysResult<SockAddr> {
        match self {
            Sock::Tcp(tcp) => {
                let peer_addr = SockAddr::from_endpoint(tcp.peer_addr()?);
                Ok(peer_addr)
            }
            Sock::Udp(udp) => {
                let peer_addr = SockAddr::from_endpoint(udp.peer_addr()?);
                Ok(peer_addr)
            }
            Sock::Unix(_) => unimplemented!(),
        }
    }

    pub fn local_addr(&self) -> SysResult<SockAddr> {
        match self {
            Sock::Tcp(tcp) => {
                let local_addr = SockAddr::from_endpoint(tcp.local_addr()?);
                Ok(local_addr)
            }
            Sock::Udp(udp) => {
                let local_addr = SockAddr::from_endpoint(udp.local_addr()?);
                Ok(local_addr)
            }
            Sock::Unix(_) => unimplemented!(),
        }
    }
    pub async fn sendto(&self, buf: &[u8], remote_addr: Option<SockAddr>) -> SysResult<usize> {
        match self {
            Sock::Tcp(tcp) => tcp.send(buf).await,
            Sock::Udp(udp) => match remote_addr {
                Some(addr) => udp.send_to(buf, addr.into_endpoint()).await,
                None => udp.send(buf).await,
            },
            Sock::Unix(_) => unimplemented!(),
        }
    }
    pub async fn recvfrom(&self, buf: &mut [u8]) -> SysResult<(usize, SockAddr)> {
        match self {
            Sock::Tcp(tcp) => {
                let bytes = tcp.recv(buf).await?;
                Ok((bytes, SockAddr::from_endpoint(tcp.peer_addr()?)))
            }
            Sock::Udp(udp) => {
                let (len, endpoint) = udp.recv_from(buf).await?;
                Ok((len, SockAddr::from_endpoint(endpoint)))
            }
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
    /// The type of socket (such as STREAM, DGRAM)
    pub types: SocketType,
    /// The core of a socket, which includes TCP, UDP, or Unix domain sockets
    pub sk: Sock,
    /// File metadata, including metadata information related to sockets
    pub meta: FileMeta,
}

unsafe impl Sync for Socket {}
unsafe impl Send for Socket {}

impl Socket {
    pub fn new(domain: SaFamily, types: SocketType, nonblock: bool) -> Self {
        let sk = match domain {
            SaFamily::AF_UNIX => Sock::Unix(UnixSocket {}),
            SaFamily::AF_INET | SaFamily::AF_INET6 => match types {
                SocketType::STREAM => Sock::Tcp(TcpSocket::new_v4()),
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

    async fn base_read_at(&self, _offset: usize, _buf: &mut [u8]) -> SyscallResult {
        unreachable!()
    }

    async fn base_write_at(&self, _offset: usize, _buf: &[u8]) -> SyscallResult {
        unreachable!()
    }

    async fn read_at(&self, _offset: usize, buf: &mut [u8]) -> SyscallResult {
        if buf.len() == 0 {
            return Ok(0);
        }
        // TODO: should add this?
        // poll_interfaces();
        let bytes = self.sk.recvfrom(buf).await.map(|e| e.0)?;
        warn!(
            "[Socket::File::read_at] expect to recv: {:?} exact: {bytes}",
            buf.len()
        );
        Ok(bytes)
    }

    async fn write_at(&self, _offset: usize, buf: &[u8]) -> SyscallResult {
        if buf.len() == 0 {
            return Ok(0);
        }
        // TODO: should add this?
        // poll_interfaces();
        let bytes = self.sk.sendto(buf, None).await?;
        warn!(
            "[Socket::File::write_at] expect to send: {:?} bytes exact: {bytes}",
            buf.len()
        );
        Ok(bytes)
    }

    async fn base_poll(&self, events: PollEvents) -> PollEvents {
        let mut res = PollEvents::empty();
        poll_interfaces();
        let netstate = self.sk.poll().await;
        if events.contains(PollEvents::IN) && netstate.readable {
            res |= PollEvents::IN;
        }
        if events.contains(PollEvents::OUT) && netstate.writable {
            res |= PollEvents::OUT;
        }
        if netstate.hangup {
            log::warn!("[Socket::bask_poll] PollEvents is hangup");
            res |= PollEvents::HUP;
        }
        log::info!("[Socket::base_poll] ret events:{res:?} {netstate:?}");
        res
    }

    fn ioctl(&self, _cmd: usize, _arg: usize) -> SyscallResult {
        log::warn!("[Socket::File::ioctl] not supported now, return 0 instead");
        Ok(0)
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
