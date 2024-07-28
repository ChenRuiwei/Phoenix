use alloc::collections::btree_map::BTreeMap;

use smoltcp::{iface::SocketHandle, wire::IpListenEndpoint};
use spin::Lazy;

use crate::Mutex;

type Port = u16;
type Fd = usize;
type Pid = usize;
/// 目前仅支持一个Port只能有一个Socket，如有冲突都是该Socket的Arc clone
/// 例如，iperf测试创建的两个Socket，AF_INET 0.0.0.0::5001 和 AF_INET6
/// ::5001 都绑定到了5001端口，本应该有两个Socket，
/// 但是这里采用了复用Sockethandle的方法
pub struct PortMap(Mutex<BTreeMap<Port, (Fd, IpListenEndpoint)>>);

pub(crate) static PORT_MAP: Lazy<PortMap> = Lazy::new(PortMap::new);

impl PortMap {
    const fn new() -> Self {
        Self(Mutex::new(BTreeMap::new()))
    }

    pub fn get(&self, port: Port) -> Option<(Fd, IpListenEndpoint)> {
        self.0.lock().get(&port).cloned()
    }

    pub fn remove(&self, port: Port) {
        self.0.lock().remove(&port);
    }

    pub fn insert(&self, port: Port, fd: Fd, listen_endpoint: IpListenEndpoint) {
        self.0.lock().insert(port, (fd, listen_endpoint));
    }
}
