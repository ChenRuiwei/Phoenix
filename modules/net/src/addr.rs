use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4};

use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address, Ipv6Address};

pub const fn from_core_ipaddr(ip: IpAddr) -> IpAddress {
    match ip {
        IpAddr::V4(ipv4) => IpAddress::Ipv4(Ipv4Address(ipv4.octets())),
        IpAddr::V6(ipv6) => IpAddress::Ipv6(Ipv6Address(ipv6.octets())),
    }
}

pub const fn into_core_ipaddr(ip: IpAddress) -> IpAddr {
    match ip {
        IpAddress::Ipv4(ipv4) => {
            IpAddr::V4(unsafe { core::mem::transmute::<[u8; 4], Ipv4Addr>(ipv4.0) })
        }
        IpAddress::Ipv6(ipv6) => {
            IpAddr::V6(unsafe { core::mem::transmute::<[u8; 16], Ipv6Addr>(ipv6.0) })
        }
    }
}

pub const fn from_core_sockaddr(addr: SocketAddr) -> IpEndpoint {
    IpEndpoint {
        addr: from_core_ipaddr(addr.ip()),
        port: addr.port(),
    }
}

pub const fn into_core_sockaddr(addr: IpEndpoint) -> SocketAddr {
    SocketAddr::new(into_core_ipaddr(addr.addr), addr.port)
}

pub fn is_unspecified(ip: IpAddress) -> bool {
    ip.as_bytes() == [0, 0, 0, 0]
}

pub const UNSPECIFIED_IP: IpAddress = IpAddress::v4(0, 0, 0, 0);
pub const UNSPECIFIED_ENDPOINT: IpEndpoint = IpEndpoint::new(UNSPECIFIED_IP, 0);
pub const LOCAL_IPV4: IpAddress = IpAddress::v4(127, 0, 0, 1);
pub const LOCAL_ENDPOINT_V4: IpEndpoint = IpEndpoint::new(LOCAL_IPV4, 0);
