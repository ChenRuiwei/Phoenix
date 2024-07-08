pub mod socket;
pub mod tcp;
pub mod udp;
mod unix;

pub const AF_UNIX: usize = 1;
pub const AF_INET: usize = 2;

#[repr(u16)]
#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub enum SocketAddressFamily {
    AF_UNIX = 1,
    AF_INET = 2,
    AF_INET6 = 10,
}

impl SocketAddressFamily {
    pub fn from_usize(value: usize) -> Result<Self, usize> {
        match value {
            1 => Ok(SocketAddressFamily::AF_UNIX),
            2 => Ok(SocketAddressFamily::AF_INET),
            10 => {
                log::error!("[AF_INET6] unsupported socket address family");
                Ok(SocketAddressFamily::AF_INET6)
            }
            _ => Err(value),
        }
    }
}

// pub enum SockType {
//     Stream,
//     Dgram,
//     Raw,
// }

bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct SocketType: i32 {
        const STREAM = 1;
        /// Supports datagrams (connectionless, unreliable messages of a fixed maximum length).
        const DGRAM = 2;
        /// Provides raw network protocol access.
        const RAW = 3;
        /// Provides a reliable datagram layer that does not guarantee ordering.
        const RDM = 4;
        /// Provides a sequenced, reliable, two-way connection-based data
        /// transmission path for datagrams of fixed maximum length;
        /// a consumer is required to read an entire packet with each input system call.
        const SEQPACKET = 5;
        /// Datagram Congestion Control Protocol socket
        const DCCP = 6;
        /// Obsolete and should not be used in new programs.
        const PACKET = 10;
        /// Set O_NONBLOCK flag on the open fd
        const NONBLOCK = 0x800;
        /// Set FD_CLOEXEC flag on the new fd
        const CLOEXEC = 0x80000;
    }
}
