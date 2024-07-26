use alloc::{sync::Arc, vec::Vec};
use core::mem::{self};

use addr::{SockAddrIn, SockAddrIn6};
use async_utils::yield_now;
use log::info;
use socket::*;
use systype::{SysError, SysResult, SyscallResult};
use vfs::pipefs::new_pipe;
use vfs_core::OpenFlags;

use super::Syscall;
use crate::{
    mm::{UserRdWrPtr, UserReadPtr, UserWritePtr},
    net::*,
    task::Task,
};
impl Syscall<'_> {
    /// creates an endpoint for communication and returns a file descriptor that
    /// refers to that endpoint. The file descriptor returned by a successful
    /// call will be the lowest-numbered file descriptor not currently open
    /// for the process.
    pub fn sys_socket(&self, domain: usize, types: i32, _protocal: usize) -> SyscallResult {
        let domain = SaFamily::try_from(domain as u16)?;
        let mut types = types;
        let mut flags = OpenFlags::empty();
        let mut nonblock = false;
        if types & NONBLOCK != 0 {
            nonblock = true;
            types &= !NONBLOCK;
            flags |= OpenFlags::O_NONBLOCK;
        }
        if types & CLOEXEC != 0 {
            types &= !CLOEXEC;
            flags |= OpenFlags::O_CLOEXEC;
        }
        let types = SocketType::try_from(types)?;
        let socket = Socket::new(domain, types, nonblock);
        let fd = self
            .task
            .with_mut_fd_table(|table| table.alloc(Arc::new(socket), flags))?;
        log::info!("[sys_socket] new socket {domain:?} {types:?} {flags:?} in fd {fd}, nonblock:{nonblock}");
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
        let local_addr = task.audit_sockaddr(addr, addrlen)?;
        let socket = task.sockfd_lookup(sockfd)?;
        info!("[sys_bind] try to bind fd{sockfd} to {local_addr}");
        socket.sk.bind(local_addr)?;
        info!(
            "[sys_bind] already bind fd{sockfd} to {}",
            socket.sk.local_addr().unwrap()
        );
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
        let remote_addr = task.audit_sockaddr(addr, addrlen)?;
        let socket = task.sockfd_lookup(sockfd)?;
        log::info!("[sys_connect] fd{sockfd} trys to connect {remote_addr}");
        socket.sk.connect(remote_addr).await?;
        // TODO:
        yield_now().await;
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
    pub async fn sys_accept(&self, sockfd: usize, addr: usize, addrlen: usize) -> SyscallResult {
        let task = self.task;
        let socket = task.sockfd_lookup(sockfd)?;
        let new_sk = socket.sk.accept().await?;
        let peer_addr = new_sk.peer_addr()?;
        log::info!("[sys_accept] peer addr: {peer_addr}");
        task.write_sockaddr(addr, addrlen, peer_addr);
        let new_socket = Arc::new(Socket::from_another(&socket, Sock::Tcp(new_sk)));
        let fd = task.with_mut_fd_table(|table| table.alloc(new_socket, OpenFlags::empty()))?;
        Ok(fd)
    }

    /// Returns the local address of the Socket corresponding to `sockfd`. The
    /// parameters `addr` and `addrlen` are both pointers.
    /// In Linux, if `addrlen` is too small, the written `addr` should be
    /// truncated. However, this is not currently done
    pub fn sys_getsockname(&self, sockfd: usize, addr: usize, addrlen: usize) -> SyscallResult {
        let task = self.task;
        let socket = task.sockfd_lookup(sockfd)?;
        let local_addr = socket.sk.local_addr()?;
        log::info!("[sys_getsockname] local addr: {local_addr:?}");
        task.write_sockaddr(addr, addrlen, local_addr);
        Ok(0)
    }

    /// Similar to `sys_getsockname`
    pub fn sys_getpeername(&self, sockfd: usize, addr: usize, addrlen: usize) -> SyscallResult {
        let task = self.task;
        let socket = task.sockfd_lookup(sockfd)?;
        let peer_addr = socket.sk.peer_addr()?;
        log::info!("[sys_getpeername] peer addr: {peer_addr:?}");
        task.write_sockaddr(addr, addrlen, peer_addr);
        Ok(0)
    }

    /// Usually used for sending UDP datagrams. If using `sys_sendto` for STEAM,
    /// `dest_addr` and `addrlen` will be ignored.
    ///
    /// On success returns the number of bytes sent
    pub async fn sys_sendto(
        &self,
        sockfd: usize,
        buf: UserReadPtr<u8>,
        len: usize,
        flags: usize,
        dest_addr: usize,
        addrlen: usize,
    ) -> SyscallResult {
        debug_assert!(flags == 0, "unsupported flags");
        let task = self.task;
        let buf = buf.into_slice(&task, len)?;
        let socket = task.sockfd_lookup(sockfd)?;
        task.set_interruptable();
        let bytes = match socket.types {
            SocketType::STREAM => {
                if dest_addr != 0 {
                    return Err(SysError::EISCONN);
                }
                socket.sk.sendto(&buf, None).await?
            }
            SocketType::DGRAM => {
                let sockaddr = if dest_addr != 0 {
                    Some(task.audit_sockaddr(dest_addr, addrlen)?)
                } else {
                    None
                };
                socket.sk.sendto(&buf, sockaddr).await?
            }
            _ => unimplemented!(),
        };
        task.set_running();
        Ok(bytes)
    }

    /// - `sockfd`: Socket descriptor, created through socket system calls.
    /// - `buf`: A pointer to a buffer used to store received data.
    /// - `len`: The length of the buffer, which is the maximum number of data
    ///   bytes received.
    /// - `flags`: Currently ignored
    /// - `src_addr`: A pointer to the sockaddr structure used to store the
    ///   sender's address information. Can be `NULL`, if the sender address is
    ///   notrequired.
    /// - `adddrlen`: A pointer to the socklen_t variable, used to store the
    ///   size of src_addr. When calling, it should be set to the size of the
    ///   src_addr structure, which will include the actual address size after
    ///   the call. Can be `NULL`, if src_addr is `NULL`.
    ///
    /// Return the number of bytes received
    pub async fn sys_recvfrom(
        &self,
        sockfd: usize,
        buf: UserWritePtr<u8>,
        len: usize,
        flags: usize,
        src_addr: usize,
        addrlen: usize,
    ) -> SyscallResult {
        debug_assert!(flags == 0, "unsupported flags");
        let task = self.task;
        let socket = task.sockfd_lookup(sockfd)?;
        info!(
            "recvfrom: {:?}, local_addr: {:?}",
            socket.sk.peer_addr(),
            socket.sk.local_addr()
        );
        let mut temp = Vec::with_capacity(len);
        unsafe { temp.set_len(len) };
        task.set_interruptable();
        // TODO: not sure if `len` is enough when call `socket.recvfrom`
        let (bytes, remote_addr) = socket.sk.recvfrom(&mut temp).await?;
        task.set_running();
        let mut buf = buf.into_mut_slice(&task, bytes)?;
        buf[..bytes].copy_from_slice(&temp[..bytes]);
        task.write_sockaddr(src_addr, addrlen, remote_addr)?;

        Ok(bytes)
    }

    /// Allow users to configure sockets
    /// But since these configurations are too detailed, they are currently not
    /// supported
    pub fn sys_setsockopt(
        &self,
        sockfd: usize,
        level: usize,
        optname: usize,
        optval: usize,
        optlen: usize,
    ) -> SyscallResult {
        // let task = self.task;
        // let socket = task.sockfd_lookup(sockfd)?;
        log::info!(
            "[sys_setsockopt] fd{sockfd} {:?} {:?} optval:{} optlen:{optlen}",
            SocketLevel::try_from(level)?,
            SocketOpt::try_from(optname)?,
            UserReadPtr::<usize>::from(optval).read(self.task)?
        );
        Ok(0)
    }

    pub fn sys_getsockopt(
        &self,
        _sockfd: usize,
        level: usize,
        optname: usize,
        optval: usize,
        optlen: usize,
    ) -> SyscallResult {
        use core::mem::size_of;
        let task = self.task;
        match SocketLevel::try_from(level)? {
            SocketLevel::SOL_SOCKET => {
                const SEND_BUFFER_SIZE: usize = 64 * 1024;
                const RECV_BUFFER_SIZE: usize = 64 * 1024;
                match SocketOpt::try_from(optname)? {
                    SocketOpt::RCVBUF => {
                        UserWritePtr::<u32>::from(optval).write(&task, RECV_BUFFER_SIZE as u32)?;
                        UserWritePtr::<u32>::from(optlen).write(&task, size_of::<u32>() as u32)?
                    }
                    SocketOpt::SNDBUF => {
                        UserWritePtr::<u32>::from(optval).write(&task, SEND_BUFFER_SIZE as u32)?;
                        UserWritePtr::<u32>::from(optlen).write(&task, size_of::<u32>() as u32)?
                    }
                    SocketOpt::ERROR => {
                        UserWritePtr::<u32>::from(optval).write(&task, 0)?;
                        UserWritePtr::<u32>::from(optlen).write(&task, size_of::<u32>() as u32)?
                    }
                    opt => {
                        log::error!(
                            "[sys_getsockopt] unsupported SOL_SOCKET opt {opt:?} optlen:{optlen}"
                        )
                    }
                };
            }
            SocketLevel::IPPROTO_IP | SocketLevel::IPPROTO_TCP => {
                const MAX_SEGMENT_SIZE: usize = 1460;
                match TcpSocketOpt::try_from(optname)? {
                    TcpSocketOpt::MAXSEG => {
                        UserWritePtr::<u32>::from(optval).write(&task, MAX_SEGMENT_SIZE as u32)?;
                        UserWritePtr::<u32>::from(optlen).write(&task, size_of::<u32>() as u32)?
                    }
                    TcpSocketOpt::NODELAY => {
                        UserWritePtr::<u32>::from(optval).write(&task, 0)?;
                        UserWritePtr::<u32>::from(optlen).write(&task, size_of::<u32>() as u32)?
                    }
                    TcpSocketOpt::INFO => {}
                    TcpSocketOpt::CONGESTION => {
                        UserWritePtr::from(optval).write_cstr(&task, "reno");
                        UserWritePtr::<u32>::from(optlen).write(&task, 4)?
                    }
                    opt => {
                        log::error!(
                            "[sys_getsockopt] unsupported IPPROTO_TCP opt {opt:?} optlen:{optlen}"
                        )
                    }
                };
            }
            SocketLevel::IPPROTO_IPV6 => todo!(),
        }
        Ok(0)
    }

    /// Unlike the `close` system call, `shutdown` allows for finer grained
    /// control over the closing behavior of connections. `shutdown` can only
    /// close the sending and receiving directions of the socket, or both at the
    /// same time
    pub fn sys_shutdown(&self, sockfd: usize, how: usize) -> SyscallResult {
        let task = self.task;
        let socket = task.sockfd_lookup(sockfd)?;
        // let how = SocketShutdownFlag::try_from(how)?;
        log::info!(
            "[sys_shutdown] sockfd:{sockfd} shutdown {}",
            match how {
                0 => "READ",
                1 => "WRITE",
                2 => "READ AND WRITE",
                _ => "Invalid argument",
            }
        );
        socket.sk.shutdown(how as u8)?;
        Ok(0)
    }

    pub fn sys_socketpair(
        &self,
        domain: usize,
        types: usize,
        protocol: usize,
        sv: UserWritePtr<[u32; 2]>,
    ) -> SyscallResult {
        let task = self.task;
        let (pipe_read, pipe_write) = new_pipe();
        let pipe = task.with_mut_fd_table(|table| {
            let fd_read = table.alloc(pipe_read, OpenFlags::empty())?;
            let fd_write = table.alloc(pipe_write, OpenFlags::empty())?;
            Ok([fd_read as u32, fd_write as u32])
        })?;
        sv.write(&task, pipe)?;
        Ok(0)
    }
}

impl Task {
    fn sockfd_lookup(&self, sockfd: usize) -> SysResult<Arc<Socket>> {
        self.with_fd_table(|table| table.get_file(sockfd))?
            .downcast_arc::<Socket>()
            .map_err(|_| SysError::ENOTSOCK)
    }
}
