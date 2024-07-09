use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use systype::SysError;

pub mod socket;
pub mod tcp;
pub mod udp;
mod unix;

pub const AF_UNIX: usize = 1;
pub const AF_INET: usize = 2;

#[repr(u16)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
/// socket address family
pub enum SaFamily {
    AF_UNIX = 1,
    /// ipv4
    AF_INET = 2,
    /// ipv6
    AF_INET6 = 10,
}

impl TryFrom<usize> for SaFamily {
    type Error = SysError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(SaFamily::AF_UNIX),
            2 => Ok(SaFamily::AF_INET),
            10 => Ok(SaFamily::AF_INET6),
            _ => Err(SysError::EINVAL),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SocketType {
    /// TCP
    STREAM = 1,
    /// UDP
    DGRAM = 2,
    RAW = 3,
    RDM = 4,
    SEQPACKET = 5,
    DCCP = 6,
    PACKET = 10,
}

impl TryFrom<i32> for SocketType {
    type Error = SysError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(SocketType::STREAM),
            2 => Ok(SocketType::DGRAM),
            3 => Ok(SocketType::RAW),
            4 => Ok(SocketType::RDM),
            5 => Ok(SocketType::SEQPACKET),
            6 => Ok(SocketType::DCCP),
            10 => Ok(SocketType::PACKET),
            _ => Err(SysError::EINVAL),
        }
    }
}

/// Set O_NONBLOCK flag on the open fd
pub const NONBLOCK: i32 = 0x800;
/// Set FD_CLOEXEC flag on the new fd
pub const CLOEXEC: i32 = 0x80000;

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
/// `SockAddr` is a superset of `SocketAddr` in `core::net` since it also
/// includes the address for socket communication between Unix processes. And it
/// is a user oriented program with a C language structure layout, used for
/// system calls to interact with users
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

impl From<SocketAddr> for SockAddr {
    fn from(value: SocketAddr) -> Self {
        match value {
            SocketAddr::V4(v4) => SockAddr::SockAddrIn(SockAddrIn {
                family: SaFamily::AF_INET as _,
                port: v4.port(),
                addr: *v4.ip(),
                zero: [0; 8],
            }),
            SocketAddr::V6(v6) => SockAddr::SockAddrIn6(SockAddrIn6 {
                family: SaFamily::AF_INET6 as _,
                port: v6.port(),
                flowinfo: v6.flowinfo(),
                addr: *v6.ip(),
                scope: v6.scope_id(),
            }),
        }
    }
}
