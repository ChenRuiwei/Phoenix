use alloc::boxed::Box;

use async_trait::async_trait;
use net::udp::UdpSocket;
use systype::{SysError, SysResult};

use super::socket::{ProtoOps, SockAddr};
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
}
