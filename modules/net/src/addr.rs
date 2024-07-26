use core::{
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address, Ipv6Address};

pub fn is_unspecified(ip: IpAddress) -> bool {
    ip.as_bytes() == [0, 0, 0, 0] || ip.as_bytes() == [0, 0, 0, 0, 0, 0]
}

// impl fmt::Display for Option<IpAddress> {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         match self {
//             Some(endpoint) => write!(f, "Some({})", endpoint),
//             None => write!(f, "None"),
//         }
//     }
// }

pub const UNSPECIFIED_IPV4: IpAddress = IpAddress::v4(0, 0, 0, 0);
pub const UNSPECIFIED_ENDPOINT_V4: IpEndpoint = IpEndpoint::new(UNSPECIFIED_IPV4, 0);
pub const UNSPECIFIED_IPV6: IpAddress = IpAddress::Ipv6(Ipv6Address::UNSPECIFIED);
pub const UNSPECIFIED_ENDPOINT_V6: IpEndpoint = IpEndpoint::new(UNSPECIFIED_IPV6, 0);
pub const LOCAL_IPV4: IpAddress = IpAddress::v4(127, 0, 0, 1);
pub const LOCAL_ENDPOINT_V4: IpEndpoint = IpEndpoint::new(LOCAL_IPV4, 0);
