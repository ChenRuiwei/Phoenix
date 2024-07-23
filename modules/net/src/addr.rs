use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address, Ipv6Address};

pub fn is_unspecified(ip: IpAddress) -> bool {
    ip.as_bytes() == [0, 0, 0, 0]
}

pub const UNSPECIFIED_IP: IpAddress = IpAddress::v4(0, 0, 0, 0);
pub const UNSPECIFIED_ENDPOINT: IpEndpoint = IpEndpoint::new(UNSPECIFIED_IP, 0);
pub const LOCAL_IPV4: IpAddress = IpAddress::v4(127, 0, 0, 1);
pub const LOCAL_ENDPOINT_V4: IpEndpoint = IpEndpoint::new(LOCAL_IPV4, 0);
