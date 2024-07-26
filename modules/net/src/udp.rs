use core::{
    ops::Deref,
    sync::atomic::{AtomicBool, Ordering},
};

use async_utils::{get_waker, suspend_now, yield_now};
use log::{debug, info, warn};
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
    addr::{LOCAL_ENDPOINT_V4, UNSPECIFIED_IPV4},
    has_signal, Mutex, NetPollState,
};

/// A UDP socket that provides POSIX-like APIs.
pub struct UdpSocket {
    handle: SocketHandle,
    local_addr: RwLock<Option<IpEndpoint>>,
    peer_addr: RwLock<Option<IpEndpoint>>,
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
        }
    }

    /// Returns the local address and port, or
    /// [`Err(NotConnected)`](AxError::NotConnected) if not connected.
    pub fn local_addr(&self) -> SysResult<IpEndpoint> {
        match self.local_addr.try_read() {
            Some(addr) => addr.ok_or(SysError::ENOTCONN),
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

    /// Binds an unbound socket to the given address and port.
    ///
    /// It's must be called before [`send_to`](Self::send_to) and
    /// [`recv_from`](Self::recv_from).
    pub fn bind(&self, mut local_addr: IpEndpoint) -> SysResult<()> {
        let mut self_local_addr = self.local_addr.write();

        if local_addr.port == 0 {
            local_addr.port = get_ephemeral_port()?;
            info!(
                "[UdpSocket::bind] No specified port, use port {}]",
                local_addr.port
            );
        }
        if self_local_addr.is_some() {
            warn!("socket bind() failed: already bound");
            return Err(SysError::EINVAL);
        }

        if let IpAddress::Ipv6(v6) = local_addr.addr {
            if v6.is_unspecified() {
                log::error!("[UdpSocket::bind] Unstable: just use ipv4 instead of ipv6 when ipv6 is unspecified");
                local_addr.addr = UNSPECIFIED_IPV4;
            }
        }
        let endpoint = IpListenEndpoint {
            addr: (!is_unspecified(local_addr.addr)).then_some(local_addr.addr),
            port: local_addr.port,
        };
        SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
            socket.bind(endpoint).map_err(|e| {
                warn!("socket bind() failed");
                match e {
                    BindError::InvalidState => SysError::EEXIST,
                    BindError::Unaddressable => SysError::EINVAL,
                }
            })
        })?;

        *self_local_addr = Some(local_addr);
        info!(
            "[Udpsocket::bind] handle {} bound on {endpoint}",
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
        let mut self_peer_addr = self.peer_addr.write();
        if self.local_addr.read().is_none() {
            info!("[UdpSocket::connect] don't have local addr, bind to UNSPECIFIED_ENDPOINT_V4");
            self.bind(UNSPECIFIED_ENDPOINT_V4)?;
        }
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
        SOCKET_SET.poll_interfaces();
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
                socket.register_recv_waker(&waker);
            }
            if !writable {
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
                "[send_impl] UDP socket {}: not bound. Use 0.0.0.0",
                self.handle
            );
            // TODO: UNSPECIFIED_ENDPOINT_V4 or LOCAL_ENDPOINT?
            self.bind(UNSPECIFIED_ENDPOINT_V4)?;
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
                SOCKET_SET.poll_interfaces();
                match f() {
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
        self.shutdown().ok();
        SOCKET_SET.remove(self.handle);
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
