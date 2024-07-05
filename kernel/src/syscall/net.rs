use alloc::sync::Arc;

use socket::*;
use systype::{SysError, SyscallResult};

use super::Syscall;
use crate::{
    mm::{audit_sockaddr, UserReadPtr},
    net::*,
};
impl Syscall<'_> {
    /// creates an endpoint for communication and returns a file descriptor that
    /// refers to that endpoint. The file descriptor returned by a successful
    /// call will be the lowest-numbered file descriptor not currently open
    /// for the process.
    pub fn sys_socket(&self, domain: usize, types: i32, protocal: usize) -> SyscallResult {
        // log::info!()
        let domain = SocketAddressFamily::from_usize(domain).map_err(|_| SysError::EINVAL)?;
        let types = SocketType::from_bits_truncate(types);
        let task = self.task;
        let socket = Socket::new(domain, types);
        // TODO:其他标志位
        let fd = task.with_mut_fd_table(|table| table.alloc(Arc::new(socket)))?;
        Ok(fd)
    }

    /// When a socket is created with socket(2), it exists in a name space
    /// (address family) but has no address assigned to it.  bind() assigns the
    /// address specified by addr to the socket referred to by the file
    /// descriptor sockfd.  addrlen specifies the size, in  bytes,  of the
    /// address structure pointed to by addr.  Traditionally, this operation is
    /// called “assigning a name to a socket”.
    pub fn sys_bind(&self, socketfd: usize, addr: usize, addrlen: usize) -> SyscallResult {
        let task = self.task;
        let socketfd = task
            .with_fd_table(|table| table.get_file(socketfd))?
            .downcast_arc::<Socket>()
            .map_err(|_| SysError::EINVAL)?;
        let sockaddr = audit_sockaddr(addr, addrlen, &task)?;

        Ok(0)
    }

    /// 将文件描述符 sockfd 引用的流 socket 标记为被动。这个 socket
    /// 后面会被用来接受来自其他（主动的）socket的连接
    pub fn sys_listen(&self, sockfd: usize, _backlog: usize) -> SyscallResult {
        Ok(0)
    }

    /// connect()系统调用将文件描述符sockfd引用的主动socket连接到地址通过addr和addrlen指定的监听socket上。
    pub fn sys_connect(&self, sockfd: usize, addr: usize, addrlen: usize) -> SyscallResult {
        Ok(0)
    }

    /// Only SOCK_STREAM can use sys_accept
    /// accept()系统调用在文件描述符sockfd引用的监听流socket上接受一个接入连接。
    /// 如果在调用accept()时不存在未决的连接，
    /// 那么调用就会阻塞直到有连接请求到达为止。
    pub fn sys_accept(&self, sockfd: usize, addr: usize, addrlen: usize) -> SyscallResult {
        Ok(0)
    }
}
