//! `SockAddr` is a C language structure layout, used for system calls to
//! interact with users. It is ** network byte order  (big endian) **
//!
//! `IpEndpoint` is host byte order

use alloc::{format, string::String};
use core::{
    fmt::{self, Display},
    panic,
};

use net::{IpAddress, IpEndpoint, IpListenEndpoint, Ipv4Address, Ipv6Address};

use super::SaFamily;

#[derive(Clone, Copy)]
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

impl fmt::Display for SockAddrIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let port = u16::from_be_bytes(self.port);
        let addr = format!(
            "{}.{}.{}.{}",
            self.addr[0], self.addr[1], self.addr[2], self.addr[3]
        );

        write!(f, "AF_INET: {}:{}", addr, port)
    }
}

#[derive(Clone, Copy)]
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

impl fmt::Display for SockAddrIn6 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let port = u16::from_be_bytes(self.port);
        let addr = format!(
            "{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
            u16::from_be_bytes([self.addr[0], self.addr[1]]),
            u16::from_be_bytes([self.addr[2], self.addr[3]]),
            u16::from_be_bytes([self.addr[4], self.addr[5]]),
            u16::from_be_bytes([self.addr[6], self.addr[7]]),
            u16::from_be_bytes([self.addr[8], self.addr[9]]),
            u16::from_be_bytes([self.addr[10], self.addr[11]]),
            u16::from_be_bytes([self.addr[12], self.addr[13]]),
            u16::from_be_bytes([self.addr[14], self.addr[15]])
        );

        write!(f, "AF_INET6: [{}]:{}", addr, port)
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
/// Unix domain socket address
pub struct SockAddrUn {
    pub family: u16,
    pub path: [u8; 108],
}

impl fmt::Display for SockAddrUn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = match self.path.iter().position(|&x| x == 0) {
            Some(pos) => String::from_utf8_lossy(&self.path[..pos]),
            None => String::from_utf8_lossy(&self.path),
        };

        write!(f, "AF_UNIX: {}", path)
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
/// `SockAddr` is a superset of `SocketAddr` in `core::net` since it also
/// includes the address for socket communication between Unix processes. And it
/// is a user oriented program with a C language structure layout, used for
/// system calls to interact with users
pub union SockAddr {
    pub family: u16,
    pub ipv4: SockAddrIn,
    pub ipv6: SockAddrIn6,
    pub unix: SockAddrUn,
}

impl fmt::Display for SockAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            match self.family {
                1 => write!(f, "{}", self.unix),  // AF_UNIX
                2 => write!(f, "{}", self.ipv4),  // AF_INET
                10 => write!(f, "{}", self.ipv6), // AF_INET6
                _ => write!(f, "Unknown address family: {}", self.family),
            }
        }
    }
}

impl SockAddr {
    /// You should make sure that `SockAddr` is IpEndpoint
    pub fn into_endpoint(&self) -> IpEndpoint {
        unsafe {
            match SaFamily::try_from(self.family).unwrap() {
                SaFamily::AF_INET => IpEndpoint::new(
                    IpAddress::Ipv4(Ipv4Address(self.ipv4.addr)),
                    u16::from_be_bytes(self.ipv4.port),
                ),
                SaFamily::AF_INET6 => IpEndpoint::new(
                    IpAddress::Ipv6(Ipv6Address(self.ipv6.addr)),
                    u16::from_be_bytes(self.ipv6.port),
                ),
                SaFamily::AF_UNIX => panic!("Shouldn't get there"),
            }
        }
    }

    pub fn into_listen_endpoint(&self) -> IpListenEndpoint {
        unsafe {
            match SaFamily::try_from(self.family).unwrap() {
                SaFamily::AF_INET => self.ipv4.into(),
                SaFamily::AF_INET6 => self.ipv6.into(),
                SaFamily::AF_UNIX => panic!("Shouldn't get there"),
            }
        }
    }

    pub fn from_endpoint(endpoint: IpEndpoint) -> Self {
        match endpoint.addr {
            IpAddress::Ipv4(v4) => Self {
                ipv4: endpoint.into(),
            },
            IpAddress::Ipv6(v6) => Self {
                ipv6: endpoint.into(),
            },
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

impl fmt::Debug for SockAddrIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Debug for SockAddrIn6 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Debug for SockAddrUn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Debug for SockAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            match self.family {
                2 => fmt::Display::fmt(&self.ipv4, f),  // AF_INET
                10 => fmt::Display::fmt(&self.ipv6, f), // AF_INET6
                1 => fmt::Display::fmt(&self.unix, f),  // AF_UNIX
                _ => write!(f, "Unknown address family: {}", self.family),
            }
        }
    }
}
