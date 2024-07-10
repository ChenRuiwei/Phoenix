use alloc::boxed::Box;

use async_trait::async_trait;
use net::{udp::UdpSocket, NetPollState};
use systype::{SysError, SysResult};

use super::{socket::ProtoOps, SockAddr};
pub struct UdpSock {
    udp: UdpSocket,
}

impl UdpSock {
    pub fn new(nonblock: bool) -> Self {
        let udp = UdpSocket::new();
        if nonblock {
            udp.set_nonblocking(true)
        }
        Self { udp }
    }
}

#[async_trait]
impl ProtoOps for UdpSock {
    fn bind(&self, myaddr: SockAddr) -> SysResult<()> {
        self.udp.bind(myaddr.into())
    }

    async fn connect(&self, vaddr: SockAddr) -> SysResult<()> {
        self.udp.connect(vaddr.into())
    }

    fn peer_addr(&self) -> SysResult<SockAddr> {
        self.udp.peer_addr().map(|addr| addr.into())
    }

    fn local_addr(&self) -> SysResult<SockAddr> {
        self.udp.local_addr().map(|addr| addr.into())
    }

    /// UDP maybe has already connected. In that case `vaddr` is `None`.
    async fn sendto(&self, buf: &[u8], vaddr: Option<SockAddr>) -> SysResult<usize> {
        match vaddr {
            Some(addr) => self.udp.send_to(buf, addr.into()).await,
            None => self.udp.send(buf).await,
        }
    }
    async fn recvfrom(&self, buf: &mut [u8]) -> SysResult<(usize, SockAddr)> {
        self.udp
            .recv_from(buf)
            .await
            .map(|(len, addr)| (len, addr.into()))
    }

    fn poll(&self) -> NetPollState {
        self.udp.poll()
    }
}
