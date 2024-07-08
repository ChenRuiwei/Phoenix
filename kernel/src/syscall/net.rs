use alloc::sync::Arc;
use core::mem;

use log::info;
use socket::*;
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::OpenFlags;

use super::Syscall;
use crate::{
    mm::{audit_sockaddr, UserRdWrPtr, UserReadPtr, UserWritePtr},
    net::*,
    task::Task,
};
impl Syscall<'_> {
    /// creates an endpoint for communication and returns a file descriptor that
    /// refers to that endpoint. The file descriptor returned by a successful
    /// call will be the lowest-numbered file descriptor not currently open
    /// for the process.
    pub fn sys_socket(&self, domain: usize, types: i32, protocal: usize) -> SyscallResult {
        let domain = SocketAddressFamily::from_usize(domain).map_err(|_| SysError::EINVAL)?;
        let types = SocketType::from_bits_truncate(types);

        log::info!("[sys_socket] {domain:?} {types:?}");
        let mut flags = OpenFlags::empty();
        if types.contains(SocketType::NONBLOCK) {
            flags |= OpenFlags::O_NONBLOCK;
        }
        if types.contains(SocketType::CLOEXEC) {
            flags |= OpenFlags::O_CLOEXEC;
        }
        let socket = Socket::new(domain, types);
        let fd = self
            .task
            .with_mut_fd_table(|table| table.alloc(Arc::new(socket), flags))?;
        Ok(fd)
    }

    /// When a socket is created with socket(2), it exists in a name space
    /// (address family) but has no address assigned to it.  bind() assigns the
    /// address specified by addr to the socket referred to by the file
    /// descriptor sockfd.  addrlen specifies the size, in  bytes,  of the
    /// address structure pointed to by addr.  Traditionally, this operation is
    /// called “assigning a name to a socket”.
    pub fn sys_bind(&self, sockfd: usize, addr: usize, addrlen: usize) -> SyscallResult {
        let task = self.task;
        let sockaddr = audit_sockaddr(addr, addrlen, task)?;
        let socket = task.sockfd_lookup(sockfd)?;
        socket.sk.bind(sockaddr)?;
        info!("[sys_bind] bind {sockfd} to {sockaddr:?}");
        Ok(0)
    }

    /// Mark the stream socket referenced by the file descriptor `sockfd` as
    /// passive. This socket will be used later to accept connections from other
    /// (active) sockets
    pub fn sys_listen(&self, sockfd: usize, _backlog: usize) -> SyscallResult {
        let socket = self.task.sockfd_lookup(sockfd)?;
        socket.sk.listen()?;
        Ok(0)
    }

    /// Connect the active socket referenced by the file descriptor `sockfd` to
    /// the listening socket specified by `addr` and `addrlen` at the address
    pub async fn sys_connect(&self, sockfd: usize, addr: usize, addrlen: usize) -> SyscallResult {
        let task = self.task;
        let sockaddr = audit_sockaddr(addr, addrlen, task)?;
        let socket = task.sockfd_lookup(sockfd)?;
        socket.sk.connect(sockaddr).await?;
        Ok(0)
    }

    /// The accept() system call accepts an incoming connection on a listening
    /// stream socket referred to by the file descriptor `sockfd`. If there are
    /// no pending connections at the time of the accept() call, the call
    /// will block until a connection request arrives. Both `addr` and
    /// `addrlen` are pointers representing peer socket address. if the addrlen
    /// pointer is not zero, it will be assigned to the actual size of the
    /// peer address.
    ///
    /// On success, the call returns the file descriptor of the newly connected
    /// socket.
    pub async fn sys_accept(
        &self,
        sockfd: usize,
        addr: usize,
        addrlen: UserRdWrPtr<usize>,
    ) -> SyscallResult {
        let task = self.task;
        let socket = task.sockfd_lookup(sockfd)?;
        let sk = socket.sk.accept().await?;
        if addr != 0 {
            let peer_addr = sk.peer_addr()?;
            if addrlen.is_null() {
                return Err(SysError::EINVAL);
            }
            let len = addrlen.read(&task)?;
            match peer_addr {
                SockAddr::SockAddrIn(v4) => {
                    if len < mem::size_of::<SockAddrIn>() {
                        return Err(SysError::EINVAL);
                    }
                    UserWritePtr::<SockAddrIn>::from(addr).write(&task, v4)?
                }
                SockAddr::SockAddrIn6(v6) => {
                    if len < mem::size_of::<SockAddrIn6>() {
                        return Err(SysError::EINVAL);
                    }
                    UserWritePtr::<SockAddrIn6>::from(addr).write(&task, v6)?
                }
                SockAddr::SockAddrUn(_) => unimplemented!(),
            }
        }
        let new_socket = Arc::new(Socket::from_another(&socket, sk));
        let fd = task.with_mut_fd_table(|table| table.alloc(new_socket, OpenFlags::empty()))?;
        Ok(fd)
    }
}

impl Task {
    fn sockfd_lookup(&self, sockfd: usize) -> SysResult<Arc<Socket>> {
        self.with_fd_table(|table| table.get_file(sockfd))?
            .downcast_arc::<Socket>()
            .map_err(|_| SysError::ENOTSOCK)
    }
}
