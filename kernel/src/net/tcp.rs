use alloc::boxed::Box;

use async_trait::async_trait;
use net::tcp::TcpSocket;
use systype::SysResult;

use super::socket::{ProtoOps, SockAddr};
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

    async fn connect(&self, vaddr: SockAddr) -> SysResult<()> {
        self.tcp.connect(vaddr.into()).await
    }
}
