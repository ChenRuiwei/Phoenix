use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use systype::SysError;

pub mod socket;
pub mod tcp;
pub mod udp;
mod unix;

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
            1 => Ok(Self::AF_UNIX),
            2 => Ok(Self::AF_INET),
            10 => Ok(Self::AF_INET6),
            _ => Err(Self::Error::EINVAL),
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
            1 => Ok(Self::STREAM),
            2 => Ok(Self::DGRAM),
            3 => Ok(Self::RAW),
            4 => Ok(Self::RDM),
            5 => Ok(Self::SEQPACKET),
            6 => Ok(Self::DCCP),
            10 => Ok(Self::PACKET),
            _ => Err(Self::Error::EINVAL),
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
    /// always set to `AF_INET`
    pub family: u16,
    /// port in network byte order
    pub port: u16,
    /// contains the host interface address in network byte order
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[allow(non_camel_case_types)]
/// used in `sys_setsockopt` and `sys_getsockopt`
pub enum SocketLevel {
    SOL_SOCKET = 1,
    IPPROTO_TCP = 6,
}

impl TryFrom<usize> for SocketLevel {
    type Error = SysError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::SOL_SOCKET),
            6 => Ok(Self::IPPROTO_TCP),
            level => {
                log::warn!("[SocketLevel] unsupported level: {level}");
                Err(Self::Error::EINVAL)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[allow(non_camel_case_types)]
/// used in `sys_setsockopt` and `sys_getsockopt`
/// https://www.cnblogs.com/cthon/p/9270778.html
pub enum SocketOpt {
    DEBUG = 1,
    REUSEADDR = 2,
    TYPE = 3,
    ERROR = 4,
    DONTROUTE = 5,
    BROADCAST = 6,
    SNDBUF = 7,
    RCVBUF = 8,
    KEEPALIVE = 9,
    OOBINLINE = 10,
    NO_CHECK = 11,
    PRIORITY = 12,
    LINGER = 13,
    BSDCOMPAT = 14,
    REUSEPORT = 15,
    PASSCRED = 16,
    PEERCRED = 17,
    RCVLOWAT = 18,
    SNDLOWAT = 19,
    RCVTIMEO_OLD = 20,
    SNDTIMEO_OLD = 21,
    SNDBUFFORCE = 32,
    RCVBUFFORCE = 33,
}

impl TryFrom<usize> for SocketOpt {
    type Error = SysError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::DEBUG),
            2 => Ok(Self::REUSEADDR),
            3 => Ok(Self::TYPE),
            4 => Ok(Self::ERROR),
            5 => Ok(Self::DONTROUTE),
            6 => Ok(Self::BROADCAST),
            7 => Ok(Self::SNDBUF),
            8 => Ok(Self::RCVBUF),
            9 => Ok(Self::KEEPALIVE),
            10 => Ok(Self::OOBINLINE),
            11 => Ok(Self::NO_CHECK),
            12 => Ok(Self::PRIORITY),
            13 => Ok(Self::LINGER),
            14 => Ok(Self::BSDCOMPAT),
            15 => Ok(Self::REUSEPORT),
            16 => Ok(Self::PASSCRED),
            17 => Ok(Self::PEERCRED),
            18 => Ok(Self::RCVLOWAT),
            19 => Ok(Self::SNDLOWAT),
            20 => Ok(Self::RCVTIMEO_OLD),
            21 => Ok(Self::SNDTIMEO_OLD),
            32 => Ok(Self::SNDBUFFORCE),
            33 => Ok(Self::RCVBUFFORCE),
            level => {
                log::warn!("[SocketOpt] unsupported option: {level}");
                Err(Self::Error::EINVAL)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[allow(non_camel_case_types)]
pub enum TcpSocketOpt {
    NODELAY = 1, // disable nagle algorithm and flush
    MAXSEG = 2,
    INFO = 11,
    CONGESTION = 13,
}

impl TryFrom<usize> for TcpSocketOpt {
    type Error = SysError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::NODELAY),
            2 => Ok(Self::MAXSEG),
            11 => Ok(Self::INFO),
            13 => Ok(Self::CONGESTION),
            level => {
                log::warn!("[TcpSocketOpt] unsupported option: {level}");
                Err(Self::Error::EINVAL)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[allow(non_camel_case_types)]
pub enum SocketShutdownFlag {
    /// further receptions will be disallowed
    SHUTRD = 0,
    /// further transmissions will be disallowed
    SHUTWR = 1,
    /// further  receptions and transmissions will be disallowed
    SHUTRDWR = 2,
}

impl TryFrom<usize> for SocketShutdownFlag {
    type Error = SysError;

    fn try_from(how: usize) -> Result<Self, Self::Error> {
        match how {
            0 => Ok(Self::SHUTRD),
            1 => Ok(Self::SHUTWR),
            2 => Ok(Self::SHUTRDWR),
            _ => Err(Self::Error::EINVAL),
        }
    }
}
