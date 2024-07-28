

use systype::SysError;
pub mod addr;
pub mod socket;
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

impl TryFrom<u16> for SaFamily {
    type Error = SysError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::AF_UNIX),
            2 => Ok(Self::AF_INET),
            10 => Ok(Self::AF_INET6),
            _ => Err(Self::Error::EINVAL),
        }
    }
}

impl From<SaFamily> for u16 {
    fn from(value: SaFamily) -> Self {
        match value {
            SaFamily::AF_UNIX => 1,
            SaFamily::AF_INET => 2,
            SaFamily::AF_INET6 => 10,
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[allow(non_camel_case_types)]
/// used in `sys_setsockopt` and `sys_getsockopt`
pub enum SocketLevel {
    /// Dummy protocol for TCP
    IPPROTO_IP = 0,
    SOL_SOCKET = 1,
    IPPROTO_TCP = 6,
    /// IPv6-in-IPv4 tunnelling
    IPPROTO_IPV6 = 41,
}

impl TryFrom<usize> for SocketLevel {
    type Error = SysError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::IPPROTO_IP),
            1 => Ok(Self::SOL_SOCKET),
            6 => Ok(Self::IPPROTO_TCP),
            41 => Ok(Self::IPPROTO_IPV6),
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
///
/// see https://www.man7.org/linux/man-pages/man7/socket.7.html
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
    SECURITY_AUTHENTICATION = 22,
    SECURITY_ENCRYPTION_TRANSPORT = 23,
    SECURITY_ENCRYPTION_NETWORK = 24,
    /// Bind this socket to a particular device like “eth0”, as specified in the
    /// passed interface name
    BINDTODEVICE = 25,
    ATTACH_FILTER = 26,
    DETACH_FILTER = 27,
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
            22 => Ok(Self::SECURITY_AUTHENTICATION),
            23 => Ok(Self::SECURITY_ENCRYPTION_TRANSPORT),
            24 => Ok(Self::SECURITY_ENCRYPTION_NETWORK),
            25 => Ok(Self::BINDTODEVICE),
            26 => Ok(Self::ATTACH_FILTER),
            27 => Ok(Self::DETACH_FILTER),
            32 => Ok(Self::SNDBUFFORCE),
            33 => Ok(Self::RCVBUFFORCE),
            opt => {
                log::warn!("[SocketOpt] unsupported option: {opt}");
                Ok(Self::DEBUG)
                // Err(Self::Error::EINVAL)
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

// #[derive(Debug, PartialEq, Eq, Clone, Copy)]
// #[allow(non_camel_case_types)]
// pub enum SocketShutdownFlag {
//     /// further receptions will be disallowed
//     SHUT_RD = 0,
//     /// further transmissions will be disallowed
//     SHUT_WR = 1,
//     /// further receptions and transmissions will be disallowed
//     SHUT_RDWR = 2,
// }

// impl TryFrom<usize> for SocketShutdownFlag {
//     type Error = SysError;

//     fn try_from(how: usize) -> Result<Self, Self::Error> {
//         match how {
//             0 => Ok(Self::SHUT_RD),
//             1 => Ok(Self::SHUT_WR),
//             2 => Ok(Self::SHUT_RDWR),
//             _ => Err(Self::Error::EINVAL),
//         }
//     }
// }
