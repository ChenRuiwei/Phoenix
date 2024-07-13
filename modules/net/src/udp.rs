use core::{
    net::SocketAddr,
    sync::atomic::{AtomicBool, Ordering},
};

use async_utils::yield_now;
use log::{debug, warn};
use smoltcp::{
    iface::SocketHandle,
    socket::udp::{self, BindError, SendError},
    wire::{IpEndpoint, IpListenEndpoint},
};
use spin::RwLock;
use systype::{SysError, SysResult};

use super::{
    addr::{from_core_sockaddr, into_core_sockaddr, is_unspecified, UNSPECIFIED_ENDPOINT},
    SocketSetWrapper, SOCKET_SET,
};
use crate::{addr::LOCAL_ENDPOINT, Mutex, NetPollState};

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
    pub fn local_addr(&self) -> SysResult<SocketAddr> {
        match self.local_addr.try_read() {
            Some(addr) => addr.map(into_core_sockaddr).ok_or(SysError::ENOTCONN),
            None => Err(SysError::ENOTCONN),
        }
    }

    /// Returns the remote address and port, or
    /// [`Err(NotConnected)`](AxError::NotConnected) if not connected.
    pub fn peer_addr(&self) -> SysResult<SocketAddr> {
        self.remote_endpoint().map(into_core_sockaddr)
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
    pub fn bind(&self, mut local_addr: SocketAddr) -> SysResult<()> {
        let mut self_local_addr = self.local_addr.write();

        if local_addr.port() == 0 {
            local_addr.set_port(get_ephemeral_port()?);
        }
        if self_local_addr.is_some() {
            warn!("socket bind() failed: already bound");
            return Err(SysError::EINVAL);
        }

        let local_endpoint = from_core_sockaddr(local_addr);
        let endpoint = IpListenEndpoint {
            addr: (!is_unspecified(local_endpoint.addr)).then_some(local_endpoint.addr),
            port: local_endpoint.port,
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

        *self_local_addr = Some(local_endpoint);
        debug!("UDP socket {}: bound on {}", self.handle, endpoint);
        Ok(())
    }

    /// Sends data on the socket to the given address. On success, returns the
    /// number of bytes written.
    pub async fn send_to(&self, buf: &[u8], remote_addr: SocketAddr) -> SysResult<usize> {
        if remote_addr.port() == 0 || remote_addr.ip().is_unspecified() {
            warn!("socket send_to() failed: invalid address");
            return Err(SysError::EINVAL);
        }
        self.send_impl(buf, from_core_sockaddr(remote_addr)).await
    }

    /// Receives a single datagram message on the socket. On success, returns
    /// the number of bytes read and the origin.
    pub async fn recv_from(&self, buf: &mut [u8]) -> SysResult<(usize, SocketAddr)> {
        self.recv_impl(|socket| match socket.recv_slice(buf) {
            Ok((len, meta)) => Ok((len, into_core_sockaddr(meta.endpoint))),
            Err(e) => {
                warn!("[udp::recv_from] socket {} failed {e:?}", self.handle);
                Err(SysError::EAGAIN)
            }
        })
        .await
    }

    /// Receives a single datagram message on the socket, without removing it
    /// from the queue. On success, returns the number of bytes read and the
    /// origin.
    pub async fn peek_from(&self, buf: &mut [u8]) -> SysResult<(usize, SocketAddr)> {
        self.recv_impl(|socket| match socket.peek_slice(buf) {
            Ok((len, meta)) => Ok((len, into_core_sockaddr(meta.endpoint))),
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
    pub fn connect(&self, addr: SocketAddr) -> SysResult<()> {
        let mut self_peer_addr = self.peer_addr.write();

        if self.local_addr.read().is_none() {
            self.bind(into_core_sockaddr(UNSPECIFIED_ENDPOINT))?;
        }

        *self_peer_addr = Some(from_core_sockaddr(addr));
        debug!("UDP socket {}: connected to {}", self.handle, addr);
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
            debug!("UDP socket {}: shutting down", self.handle);
            socket.close();
        });
        SOCKET_SET.poll_interfaces();
        Ok(())
    }

    /// Whether the socket is readable or writable.
    pub fn poll(&self) -> NetPollState {
        if self.local_addr.read().is_none() {
            return NetPollState {
                readable: false,
                writable: false,
            };
        }
        SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| NetPollState {
            readable: socket.can_recv(),
            writable: socket.can_send(),
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
            // TODO: UNSPECIFIED_ENDPOINT or LOCAL_ENDPOINT?
            self.bind(into_core_sockaddr(UNSPECIFIED_ENDPOINT))?;
        }

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
                        Err(SysError::EAGAIN)
                    }
                })
            })
            .await?;
        log::info!("[udp::send_impl] send {bytes}bytes to {remote_endpoint:?}");
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
                    log::info!("[recv_impl] no more data");
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
                        // TODO:判断是否有信号
                        yield_now().await
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
