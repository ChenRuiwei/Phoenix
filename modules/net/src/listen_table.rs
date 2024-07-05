use alloc::{boxed::Box, collections::VecDeque};
use core::ops::{Deref, DerefMut};

use log::*;
use smoltcp::{
    iface::{SocketHandle, SocketSet},
    socket::tcp::{self, State},
    wire::{IpAddress, IpEndpoint, IpListenEndpoint},
};
use systype::{SysError, SysResult, SyscallResult};

use super::{SocketSetWrapper, LISTEN_QUEUE_SIZE, SOCKET_SET};
use crate::Mutex;

const PORT_NUM: usize = 65536;

struct ListenTableEntry {
    /// 所监听的 IP 地址和端口号
    listen_endpoint: IpListenEndpoint,
    /// 在TCP连接建立过程中，服务器会收到客户端发送的SYN包，
    /// 并将其放入SYN队列中等待处理
    syn_queue: VecDeque<SocketHandle>,
}

impl ListenTableEntry {
    pub fn new(listen_endpoint: IpListenEndpoint) -> Self {
        Self {
            listen_endpoint,
            syn_queue: VecDeque::with_capacity(LISTEN_QUEUE_SIZE),
        }
    }

    #[inline]
    fn can_accept(&self, dst: IpAddress) -> bool {
        match self.listen_endpoint.addr {
            Some(addr) => addr == dst,
            None => true,
        }
    }
}

impl Drop for ListenTableEntry {
    fn drop(&mut self) {
        for &handle in &self.syn_queue {
            SOCKET_SET.remove(handle);
        }
    }
}

/// 用于管理TCP监听端口的表，每个索引对应一个特定的端口号
///
/// 使用数组的方式，可以通过端口号直接访问对应的监听条目，提高查找效率。
/// Mutex 用于确保线程安全，因为在多线程环境下，
/// 多个线程可能会同时访问和修改监听端口的状态。
pub struct ListenTable {
    tcp: Box<[Mutex<Option<Box<ListenTableEntry>>>]>,
}

impl ListenTable {
    pub fn new() -> Self {
        let tcp = unsafe {
            let mut buf = Box::new_uninit_slice(PORT_NUM);
            for i in 0..PORT_NUM {
                buf[i].write(Mutex::new(None));
            }
            buf.assume_init()
        };
        Self { tcp }
    }

    pub fn can_listen(&self, port: u16) -> bool {
        self.tcp[port as usize].lock().is_none()
    }

    pub fn listen(&self, listen_endpoint: IpListenEndpoint) -> SysResult<()> {
        let port = listen_endpoint.port;
        assert_ne!(port, 0);
        let mut entry = self.tcp[port as usize].lock();
        if entry.is_none() {
            *entry = Some(Box::new(ListenTableEntry::new(listen_endpoint)));
            Ok(())
        } else {
            warn!("socket listen() failed");
            Err(SysError::EADDRINUSE)
        }
    }

    pub fn unlisten(&self, port: u16) {
        debug!("TCP socket unlisten on {}", port);
        *self.tcp[port as usize].lock() = None;
    }

    pub fn can_accept(&self, port: u16) -> AxResult<bool> {
        if let Some(entry) = self.tcp[port as usize].lock().deref() {
            Ok(entry.syn_queue.iter().any(|&handle| is_connected(handle)))
        } else {
            ax_err!(InvalidInput, "socket accept() failed: not listen")
        }
    }

    /// 检查端口上的SYN队列，找到已经建立连接的句柄，并将其从队列中取出，
    /// 返回给调用者。
    pub fn accept(&self, port: u16) -> SysResult<(SocketHandle, (IpEndpoint, IpEndpoint))> {
        if let Some(entry) = self.tcp[port as usize].lock().deref_mut() {
            let syn_queue = &mut entry.syn_queue;
            let (idx, addr_tuple) = syn_queue
                .iter()
                .enumerate()
                .find_map(|(idx, &handle)| {
                    is_connected(handle).then(|| (idx, get_addr_tuple(handle)))
                })
                .ok_or(SysError::EAGAIN)?; // wait for connection

            // 记录慢速SYN队列遍历的警告信息是为了监控和诊断性能问题
            // 理想情况: 如果网络连接正常，
            // SYN队列中的连接请求应尽快完成三次握手并从队列前端被取出。因此，
            // 最常见的情况是已连接的句柄在队列的前端，即索引为0。
            // 异常情况: 如果队列中第一个元素（索引为0）的连接请求没有完成，
            // 而后续的某个连接请求已经完成，这可能表明存在性能问题或异常情况,如网络延迟、
            // 资源争用
            if idx > 0 {
                warn!(
                    "slow SYN queue enumeration: index = {}, len = {}!",
                    idx,
                    syn_queue.len()
                );
            }
            let handle = syn_queue.swap_remove_front(idx).unwrap();
            Ok((handle, addr_tuple))
        } else {
            warn!("socket accept() failed: not listen");
            Err(SysError::EINVAL)
        }
    }

    pub fn incoming_tcp_packet(
        &self,
        src: IpEndpoint,
        dst: IpEndpoint,
        sockets: &mut SocketSet<'_>,
    ) {
        if let Some(entry) = self.tcp[dst.port as usize].lock().deref_mut() {
            if !entry.can_accept(dst.addr) {
                // not listening on this address
                return;
            }
            if entry.syn_queue.len() >= LISTEN_QUEUE_SIZE {
                // SYN queue is full, drop the packet
                warn!("SYN queue overflow!");
                return;
            }
            let mut socket = SocketSetWrapper::new_tcp_socket();
            if socket.listen(entry.listen_endpoint).is_ok() {
                let handle = sockets.add(socket);
                debug!(
                    "TCP socket {}: prepare for connection {} -> {}",
                    handle, src, entry.listen_endpoint
                );
                entry.syn_queue.push_back(handle);
            }
        }
    }
}

fn is_connected(handle: SocketHandle) -> bool {
    SOCKET_SET.with_socket::<tcp::Socket, _, _>(handle, |socket| {
        !matches!(socket.state(), State::Listen | State::SynReceived)
    })
}

fn get_addr_tuple(handle: SocketHandle) -> (IpEndpoint, IpEndpoint) {
    SOCKET_SET.with_socket::<tcp::Socket, _, _>(handle, |socket| {
        (
            socket.local_endpoint().unwrap(),
            socket.remote_endpoint().unwrap(),
        )
    })
}
