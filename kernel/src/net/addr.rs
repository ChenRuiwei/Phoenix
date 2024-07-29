//! `SockAddr` is a C language structure layout, used for system calls to
//! interact with users. It is ** network byte order  (big endian) **
//!
//! `IpEndpoint` is host byte order

use core::panic;

use net::{IpAddress, IpEndpoint, IpListenEndpoint, Ipv4Address, Ipv6Address};

use super::SaFamily;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
/// IPv4 address
pub struct SockAddrIn {
    /// always set to `AF_INET`
    pub family: u16,
    /// port in network byte order
    pub port: [u8; 2],
    /// address in network byte order
    pub addr: [u8; 4],
    pub zero: [u8; 8],
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
/// IPv6 address
pub struct SockAddrIn6 {
    pub family: u16,
    /// port in network byte order (big endian)
    pub port: [u8; 2],
    pub flowinfo: u32,
    pub addr: [u8; 16],
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

impl Into<IpEndpoint> for SockAddr {
    fn into(self) -> IpEndpoint {
        match self {
            SockAddr::SockAddrIn(v4) => IpEndpoint::new(
                IpAddress::Ipv4(Ipv4Address(v4.addr)),
                u16::from_be_bytes(v4.port),
            ),
            SockAddr::SockAddrIn6(v6) => IpEndpoint::new(
                IpAddress::Ipv6(Ipv6Address(v6.addr)),
                u16::from_be_bytes(v6.port),
            ),
            SockAddr::SockAddrUn(_) => {
                panic!("unix addr isn't Internet. You shouldn't convert to IpEndpoint")
            }
        }
    }
}

impl From<SockAddrIn> for IpEndpoint {
    fn from(v4: SockAddrIn) -> Self {
        IpEndpoint::new(
            IpAddress::Ipv4(Ipv4Address(v4.addr)),
            u16::from_be_bytes(v4.port),
        )
    }
}

impl From<SockAddrIn6> for IpEndpoint {
    fn from(v6: SockAddrIn6) -> Self {
        IpEndpoint::new(
            IpAddress::Ipv6(Ipv6Address(v6.addr)),
            u16::from_be_bytes(v6.port),
        )
    }
}

impl From<IpEndpoint> for SockAddrIn {
    fn from(v4: IpEndpoint) -> Self {
        if let IpAddress::Ipv4(v4_addr) = v4.addr {
            return Self {
                family: SaFamily::AF_INET.into(),
                port: v4.port.to_be_bytes(),
                addr: unsafe { core::mem::transmute::<Ipv4Address, [u8; 4]>(v4_addr) },
                zero: [0; 8],
            };
        } else {
            // this won't happen
            panic!();
        }
    }
}

impl From<IpEndpoint> for SockAddrIn6 {
    fn from(v6: IpEndpoint) -> Self {
        if let IpAddress::Ipv6(v6_addr) = v6.addr {
            return Self {
                family: SaFamily::AF_INET6.into(),
                port: v6.port.to_be_bytes(),
                flowinfo: 0,
                addr: unsafe { core::mem::transmute::<Ipv6Address, [u8; 16]>(v6_addr) },
                scope: 0,
            };
        } else {
            panic!();
        }
    }
}

impl From<IpEndpoint> for SockAddr {
    fn from(value: IpEndpoint) -> Self {
        match value.addr {
            IpAddress::Ipv4(_v4) => Self::SockAddrIn(value.into()),
            IpAddress::Ipv6(_v6) => Self::SockAddrIn6(value.into()),
        }
    }
}

impl From<SockAddrIn> for IpListenEndpoint {
    fn from(v4: SockAddrIn) -> Self {
        let addr = Ipv4Address(v4.addr);
        let addr = if addr.is_unspecified() {
            None
        } else {
            Some(IpAddress::Ipv4(addr))
        };
        Self {
            addr,
            port: u16::from_be_bytes(v4.port),
        }
    }
}

impl From<SockAddrIn6> for IpListenEndpoint {
    fn from(v6: SockAddrIn6) -> Self {
        let addr = Ipv6Address(v6.addr);
        let addr = if addr.is_unspecified() {
            None
        } else {
            Some(IpAddress::Ipv6(addr))
        };
        Self {
            addr,
            port: u16::from_be_bytes(v6.port),
        }
    }
}
