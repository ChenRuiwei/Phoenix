use systype::SysResult;

use super::socket::{ProtoOps, SockAddr};

pub struct UnixSock {}

impl ProtoOps for UnixSock {
    fn bind(&self, _myaddr: SockAddr) -> SysResult<()> {
        unimplemented!()
    }
}
