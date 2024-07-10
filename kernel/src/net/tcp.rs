use alloc::{boxed::Box, sync::Arc};

use async_trait::async_trait;
use net::{tcp::TcpSocket, NetPollState};
use systype::SysResult;

use super::{socket::ProtoOps, SockAddr};
pub struct TcpSock {
    tcp: TcpSocket,
}

impl TcpSock {
    pub fn new(nonblock: bool) -> Self {
        let tcp = TcpSocket::new();
        if nonblock {
            tcp.set_nonblocking(true)
        }
        Self { tcp }
    }
}

#[async_trait]
impl ProtoOps for TcpSock {
    fn bind(&self, myaddr: SockAddr) -> SysResult<()> {
        self.tcp.bind(myaddr.into())
    }

    fn listen(&self) -> SysResult<()> {
        self.tcp.listen()
    }

    async fn accept(&self) -> SysResult<Arc<dyn ProtoOps>> {
        let tcp = self.tcp.accept().await?;
        Ok(Arc::new(Self { tcp }))
    }

    async fn connect(&self, vaddr: SockAddr) -> SysResult<()> {
        self.tcp.connect(vaddr.into()).await
    }

    fn peer_addr(&self) -> SysResult<SockAddr> {
        self.tcp.peer_addr().map(|addr| addr.into())
    }

    fn local_addr(&self) -> SysResult<SockAddr> {
        self.tcp.local_addr().map(|addr| addr.into())
    }

    /// since TCP has already connected, we needn't remote addr
    async fn sendto(&self, buf: &[u8], _vaddr: Option<SockAddr>) -> SysResult<usize> {
        self.tcp.send(buf).await
    }

    async fn recvfrom(&self, buf: &mut [u8]) -> SysResult<(usize, SockAddr)> {
        let bytes = self.tcp.recv(buf).await?;
        let peer_addr = self.peer_addr()?;
        Ok((bytes, peer_addr))
    }

    fn poll(&self) -> NetPollState {
        self.tcp.poll()
    }
}
