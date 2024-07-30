use core::{
    cell::UnsafeCell,
    ops::Deref,
    sync::atomic::{AtomicBool, Ordering},
};

use async_utils::{get_waker, suspend_now, yield_now};
use log::{debug, error, info, warn};
use smoltcp::{
    iface::SocketHandle,
    socket::udp::{self, BindError, SendError},
    wire::{IpAddress, IpEndpoint, IpListenEndpoint},
};
use spin::RwLock;
use systype::{SysError, SysResult};

use super::{
    addr::{is_unspecified, UNSPECIFIED_ENDPOINT_V4},
    SocketSetWrapper, SOCKET_SET,
};
use crate::{
    addr::{
        to_endpoint, LOCAL_ENDPOINT_V4, LOCAL_IPV4, UNSPECIFIED_IPV4, UNSPECIFIED_LISTEN_ENDPOINT,
    },
    has_signal,
    portmap::PORT_MAP,
    Mutex, NetPollState,
};

/// A UDP socket that provides POSIX-like APIs.
pub struct UdpSocket {
    /// Handle obtained after adding the newly created socket to SOCKET_SET.
    handle: SocketHandle,
    /// Local address and port. Uses RwLock for thread-safe read/write access.
    local_addr: RwLock<Option<IpListenEndpoint>>,
    /// Remote address and port. Uses RwLock for thread-safe read/write access.
    peer_addr: RwLock<Option<IpEndpoint>>,
    /// Indicates if the socket is in nonblocking mode. Uses AtomicBool for
    /// thread-safe access.
    nonblock: AtomicBool,
}

impl UdpSocket {
    /// Creates a new UDP socket.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let socket = SocketSetWrapper::new_udp_socket();
        let handle = SOCKET_SET.add(socket);
        Self {
            handle,
            local_addr: RwLock::new(None),
            peer_addr: RwLock::new(None),
            nonblock: AtomicBool::new(false),
            // overridden: AtomicBool::new(false),
        }
    }

    /// Returns the local address and port, or
    /// [`Err(NotConnected)`](AxError::NotConnected) if not connected.
    pub fn local_addr(&self) -> SysResult<IpEndpoint> {
        match self.local_addr.try_read() {
            Some(addr) => addr.ok_or(SysError::ENOTCONN).map(to_endpoint),
            None => Err(SysError::ENOTCONN),
        }
    }

    /// Returns the remote address and port, or
    /// [`Err(NotConnected)`](AxError::NotConnected) if not connected.
    pub fn peer_addr(&self) -> SysResult<IpEndpoint> {
        self.remote_endpoint()
    }

    /// Returns whether this socket is in nonblocking mode.
    #[inline]
    pub fn is_nonblocking(&self) -> bool {
        self.nonblock.load(Ordering::Acquire)
    }

    /// Moves this UDP socket into or out of nonblocking mode.
    ///
    /// This will result in `recv`, `recv_from`, `send`, and `send_to`
    /// operations becoming nonblocking, i.e., immediately returning from their
    /// calls. If the IO operation is successful, `Ok` is returned and no
    /// further action is required. If the IO operation could not be completed
    /// and needs to be retried, an error with kind
    /// [`Err(WouldBlock)`](AxError::WouldBlock) is returned.
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) {
        self.nonblock.store(nonblocking, Ordering::Release);
    }

    pub fn check_bind(&self, fd: usize, mut bound_addr: IpListenEndpoint) -> Option<usize> {
        // 查看是否已经用过该端口和地址。可以将两个UDP套接字绑定到同一个端口，
        // 但它们需要绑定到不同的地址
        if let Some((fd, prev_bound_addr)) = PORT_MAP.get(bound_addr.port) {
            if bound_addr == prev_bound_addr {
                warn!("[UdpSocket::bind] The port is already used by another socket. Reuse the Arc of {fd}");
                // SOCKET_SET.remove(self.handle);
                // self.overridden.store(true, Ordering::SeqCst);
                // 这个check_bind函数到这里执行之后，该Udp复用原来的Socket
                // File，所以该UdpSocket待会儿会立即drop掉
                return Some(fd);
            }
        }
        if bound_addr.port == 0 {
            bound_addr.port = get_ephemeral_port().unwrap();
            info!(
                "[UdpSocket::bind] No specified port, use port {}",
                bound_addr.port
            );
        }
        PORT_MAP.insert(bound_addr.port, fd, bound_addr);
        None
    }

    /// Binds an unbound socket to the given address and port.
    ///
    /// It's must be called before [`send_to`](Self::send_to) and
    /// [`recv_from`](Self::recv_from).
    pub fn bind(&self, mut bound_addr: IpListenEndpoint) -> SysResult<()> {
        let mut self_local_addr = self.local_addr.write();

        if bound_addr.port == 0 {
            bound_addr.port = get_ephemeral_port()?;
            info!(
                "[UdpSocket::bind] No specified port, use port {}",
                bound_addr.port
            );
        }
        if self_local_addr.is_some() {
            warn!("socket bind() failed: The socket is already bound to an address.");
            return Err(SysError::EINVAL);
        }

        // if let IpAddress::Ipv6(v6) = bound_addr.addr {
        //     if v6.is_unspecified() {
        //         log::warn!("[UdpSocket::bind] Unstable: just use 127.0.0.1 instead of
        // ipv6 when ipv6 is unspecified");         bound_addr.addr =
        // LOCAL_IPV4;     }
        // }
        // let endpoint = IpListenEndpoint {
        //     addr: (!is_unspecified(bound_addr.addr)).then_some(bound_addr.addr),
        //     port: bound_addr.port,
        // };
        SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
            socket.bind(bound_addr).map_err(|e| {
                warn!("socket bind() failed");
                match e {
                    BindError::InvalidState => SysError::EEXIST,
                    BindError::Unaddressable => SysError::EINVAL,
                }
            })
        })?;

        *self_local_addr = Some(bound_addr);
        info!(
            "[Udpsocket::bind] handle {} bound on {bound_addr}",
            self.handle
        );
        Ok(())
    }

    /// Sends data on the socket to the given address. On success, returns the
    /// number of bytes written.
    pub async fn send_to(&self, buf: &[u8], remote_addr: IpEndpoint) -> SysResult<usize> {
        if remote_addr.port == 0 || remote_addr.addr.is_unspecified() {
            warn!("socket send_to() failed: invalid remote address");
            return Err(SysError::EINVAL);
        }
        self.send_impl(buf, remote_addr).await
    }

    /// Receives a single datagram message on the socket. On success, returns
    /// the number of bytes read and the origin.
    pub async fn recv_from(&self, buf: &mut [u8]) -> SysResult<(usize, IpEndpoint)> {
        self.recv_impl(|socket| match socket.recv_slice(buf) {
            Ok((len, meta)) => Ok((len, meta.endpoint)),
            Err(e) => {
                warn!("[UdpSocket::recv_from] socket {} failed {e:?}", self.handle);
                Err(SysError::EAGAIN)
            }
        })
        .await
    }

    /// Receives a single datagram message on the socket, without removing it
    /// from the queue. On success, returns the number of bytes read and the
    /// origin.
    pub async fn peek_from(&self, buf: &mut [u8]) -> SysResult<(usize, IpEndpoint)> {
        self.recv_impl(|socket| match socket.peek_slice(buf) {
            Ok((len, meta)) => Ok((len, meta.endpoint)),
            Err(_) => {
                warn!("socket recv_from() failed");
                Err(SysError::EAGAIN)
            }
        })
        .await
    }

    /// Connects this UDP socket to a remote address, allowing the `send` and
    /// `recv` to be used to send data and also applies filters to only receive
    /// data from the specified address.
    ///
    /// The local port will be generated automatically if the socket is not
    /// bound. It's must be called before [`send`](Self::send) and
    /// [`recv`](Self::recv).
    pub fn connect(&self, addr: IpEndpoint) -> SysResult<()> {
        if self.local_addr.read().is_none() {
            info!(
                "[UdpSocket::connect] don't have local addr, bind to UNSPECIFIED_LISTEN_ENDPOINT"
            );
            self.bind(UNSPECIFIED_LISTEN_ENDPOINT)?;
        }
        let mut self_peer_addr = self.peer_addr.write();
        *self_peer_addr = Some(addr);
        info!(
            "[UdpSocket::connect] handle {} local {} connected to remote {}",
            self.handle,
            self.local_addr.read().deref().unwrap(),
            addr
        );
        Ok(())
    }

    /// Sends data on the socket to the remote address to which it is connected.
    pub async fn send(&self, buf: &[u8]) -> SysResult<usize> {
        let remote_endpoint = self.remote_endpoint()?;
        self.send_impl(buf, remote_endpoint).await
    }

    /// Receives a single datagram message on the socket from the remote address
    /// to which it is connected. On success, returns the number of bytes read.
    pub async fn recv(&self, buf: &mut [u8]) -> SysResult<usize> {
        let remote_endpoint = self.remote_endpoint()?;
        self.recv_impl(|socket| {
            let (len, meta) = socket.recv_slice(buf).map_err(|_| {
                warn!("socket recv()  failed");
                SysError::EAGAIN
            })?;
            if !is_unspecified(remote_endpoint.addr) && remote_endpoint.addr != meta.endpoint.addr {
                return Err(SysError::EAGAIN);
            }
            if remote_endpoint.port != 0 && remote_endpoint.port != meta.endpoint.port {
                return Err(SysError::EAGAIN);
            }
            Ok(len)
        })
        .await
    }

    /// Close the socket.
    pub fn shutdown(&self) -> SysResult<()> {
        SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
            warn!(
                "UDP socket {}: shutting down, remote {:?}",
                self.handle,
                self.peer_addr()
            );
            socket.close();
        });
        let timestamp = SOCKET_SET.poll_interfaces();
        SOCKET_SET.check_poll(timestamp);
        Ok(())
    }

    /// Whether the socket is readable or writable.
    pub async fn poll(&self) -> NetPollState {
        if self.local_addr.read().is_none() {
            return NetPollState {
                readable: false,
                writable: false,
                hangup: false,
            };
        }
        let waker = get_waker().await;
        SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
            let readable = socket.can_recv();
            let writable = socket.can_send();
            if !readable {
                log::info!("[UdpSocket::poll] not readable, register recv waker");
                socket.register_recv_waker(&waker);
            }
            if !writable {
                log::info!("[UdpSocket::poll] not writable, register send waker");
                socket.register_send_waker(&waker);
            }
            NetPollState {
                readable,
                writable,
                hangup: false,
            }
        })
    }
}

/// Private methods
impl UdpSocket {
    fn remote_endpoint(&self) -> SysResult<IpEndpoint> {
        match self.peer_addr.try_read() {
            Some(addr) => addr.ok_or(SysError::ENOTCONN),
            None => Err(SysError::ENOTCONN),
        }
    }

    async fn send_impl(&self, buf: &[u8], remote_endpoint: IpEndpoint) -> SysResult<usize> {
        if self.local_addr.read().is_none() {
            warn!(
                "[send_impl] UDP socket {}: not bound. Use 127.0.0.1",
                self.handle
            );
            self.bind(UNSPECIFIED_LISTEN_ENDPOINT)?;
        }
        let waker = get_waker().await;
        let bytes = self
            .block_on(|| {
                SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
                    if socket.can_send() {
                        socket
                            .send_slice(buf, remote_endpoint)
                            .map_err(|e| match e {
                                SendError::BufferFull => {
                                    warn!("socket send() failed, {e:?}");
                                    socket.register_send_waker(&waker);
                                    SysError::EAGAIN
                                }
                                SendError::Unaddressable => {
                                    warn!("socket send() failed, {e:?}");
                                    SysError::ECONNREFUSED
                                }
                            })?;
                        Ok(buf.len())
                    } else {
                        // tx buffer is full
                        info!(
                            "[UdpSocket::send_impl] handle{} can't send now, tx buffer is full",
                            self.handle
                        );
                        socket.register_send_waker(&waker);
                        Err(SysError::EAGAIN)
                    }
                })
            })
            .await?;
        log::info!("[UdpSocket::send_impl] send {bytes}bytes to {remote_endpoint:?}");
        Ok(bytes)
    }

    async fn recv_impl<F, T>(&self, mut op: F) -> SysResult<T>
    where
        F: FnMut(&mut udp::Socket) -> SysResult<T>,
    {
        if self.local_addr.read().is_none() {
            warn!("socket send() failed");
            return Err(SysError::ENOTCONN);
        }
        let waker = get_waker().await;
        self.block_on(|| {
            SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
                if socket.can_recv() {
                    // data available
                    op(socket)
                } else if !socket.is_open() {
                    // TODO: I suppose that this would't happen
                    warn!("UDP socket {}: recv() failed: not connected", self.handle);
                    Err(SysError::ENOTCONN)
                } else {
                    // no more data
                    log::info!("[recv_impl] no more data, register waker and suspend now");
                    socket.register_recv_waker(&waker);
                    Err(SysError::EAGAIN)
                }
            })
        })
        .await
    }

    async fn block_on<F, T>(&self, mut f: F) -> SysResult<T>
    where
        F: FnMut() -> SysResult<T>,
    {
        if self.is_nonblocking() {
            f()
        } else {
            loop {
                let timestamp = SOCKET_SET.poll_interfaces();
                let ret = f();
                SOCKET_SET.check_poll(timestamp);
                match ret {
                    Ok(t) => return Ok(t),
                    Err(SysError::EAGAIN) => {
                        suspend_now().await;
                        if has_signal() {
                            warn!("[UdpSocket::block_on] has signal");
                            return Err(SysError::EINTR);
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }
}

impl Drop for UdpSocket {
    fn drop(&mut self) {
        // if self.overridden.load(Ordering::Relaxed) {
        //     return;
        // }
        self.shutdown().ok();
        SOCKET_SET.remove(self.handle);
        if let Ok(addr) = self.local_addr() {
            PORT_MAP.remove(addr.port);
        }
    }
}

fn get_ephemeral_port() -> SysResult<u16> {
    const PORT_START: u16 = 0xc000;
    const PORT_END: u16 = 0xffff;
    static CURR: Mutex<u16> = Mutex::new(PORT_START);
    let mut curr = CURR.lock();

    let port = *curr;
    if *curr == PORT_END {
        *curr = PORT_START;
    } else {
        *curr += 1;
    }
    Ok(port)
}
