use core::{
    cell::UnsafeCell,
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
    task::{Context, Poll, Waker},
};

use async_utils::{get_waker, suspend_now, yield_now};
use log::*;
use smoltcp::{
    iface::SocketHandle,
    socket::tcp::{self, ConnectError, State},
    wire::{IpAddress, IpEndpoint, IpListenEndpoint},
};
use systype::*;
use timer::timelimited_task::ksleep_ms;

use super::{
    addr::{is_unspecified, UNSPECIFIED_ENDPOINT_V4},
    SocketSetWrapper, ETH0, LISTEN_TABLE, SOCKET_SET,
};
use crate::{
    addr::UNSPECIFIED_IPV4, has_signal, Mutex, NetPollState, RCV_SHUTDOWN, SEND_SHUTDOWN,
    SHUTDOWN_MASK, SHUT_RD, SHUT_RDWR, SHUT_WR, TCP_RX_BUF_LEN, TCP_TX_BUF_LEN,
};

// State transitions:
// CLOSED -(connect)-> BUSY -> CONNECTING -> CONNECTED -(shutdown)-> BUSY ->
// CLOSED       |
//       |-(listen)-> BUSY -> LISTENING -(shutdown)-> BUSY -> CLOSED
//       |
//        -(bind)-> BUSY -> CLOSED
const STATE_CLOSED: u8 = 0;
const STATE_BUSY: u8 = 1;
const STATE_CONNECTING: u8 = 2;
const STATE_CONNECTED: u8 = 3;
const STATE_LISTENING: u8 = 4;

/// A TCP socket that provides POSIX-like APIs.
///
/// - [`connect`] is for TCP clients.
/// - [`bind`], [`listen`], and [`accept`] are for TCP servers.
/// - Other methods are for both TCP clients and servers.
///
/// [`connect`]: TcpSocket::connect
/// [`bind`]: TcpSocket::bind
/// [`listen`]: TcpSocket::listen
/// [`accept`]: TcpSocket::accept
pub struct TcpSocket {
    /// Manages the state of the socket using an atomic u8 for lock-free
    /// management.
    state: AtomicU8,
    /// Indicates whether the read or write directions of the socket have been
    /// explicitly shut down. This does not represent the connection state.
    /// Once shut down, the socket cannot be reconnected via `connect`.
    shutdown: UnsafeCell<u8>,
    /// An optional handle to the socket, managed within an UnsafeCell for
    /// interior mutability.
    handle: UnsafeCell<Option<SocketHandle>>,
    /// Stores the local IP endpoint of the socket, using UnsafeCell for
    /// interior mutability.
    local_addr: UnsafeCell<IpEndpoint>,
    /// Stores the peer IP endpoint of the socket, using UnsafeCell for interior
    /// mutability.
    peer_addr: UnsafeCell<IpEndpoint>,
    /// Indicates whether the socket is in non-blocking mode, using an atomic
    /// boolean for thread-safe access.
    nonblock: AtomicBool,
}

unsafe impl Sync for TcpSocket {}

impl TcpSocket {
    /// Creates a new TCP socket.
    ///
    /// 此时并没有加到SocketSet中（还没有handle），在connect/listen中才会添加
    pub const fn new_v4() -> Self {
        Self {
            state: AtomicU8::new(STATE_CLOSED),
            shutdown: UnsafeCell::new(0),
            handle: UnsafeCell::new(None),
            local_addr: UnsafeCell::new(UNSPECIFIED_ENDPOINT_V4),
            peer_addr: UnsafeCell::new(UNSPECIFIED_ENDPOINT_V4),
            nonblock: AtomicBool::new(false),
        }
    }

    /// Creates a new TCP socket that is already connected.
    const fn new_connected(
        handle: SocketHandle,
        local_addr: IpEndpoint,
        peer_addr: IpEndpoint,
    ) -> Self {
        Self {
            state: AtomicU8::new(STATE_CONNECTED),
            shutdown: UnsafeCell::new(0),
            handle: UnsafeCell::new(Some(handle)),
            local_addr: UnsafeCell::new(local_addr),
            peer_addr: UnsafeCell::new(peer_addr),
            nonblock: AtomicBool::new(false),
        }
    }

    /// Returns the local address and port, or
    /// [`Err(NotConnected)`](AxError::NotConnected) if not connected.
    #[inline]
    pub fn local_addr(&self) -> SysResult<IpEndpoint> {
        match self.get_state() {
            STATE_CONNECTED | STATE_LISTENING | STATE_CLOSED => {
                Ok(unsafe { self.local_addr.get().read() })
            }
            _ => Err(SysError::ENOTCONN),
        }
    }

    /// Returns the remote address and port, or
    /// [`Err(NotConnected)`](AxError::NotConnected) if not connected.
    #[inline]
    pub fn peer_addr(&self) -> SysResult<IpEndpoint> {
        match self.get_state() {
            STATE_CONNECTED | STATE_LISTENING => Ok(unsafe { self.peer_addr.get().read() }),
            _ => Err(SysError::ENOTCONN),
        }
    }

    /// Returns whether this socket is in nonblocking mode.
    #[inline]
    pub fn is_nonblocking(&self) -> bool {
        self.nonblock.load(Ordering::Acquire)
    }

    /// Moves this TCP stream into or out of nonblocking mode.
    ///
    /// This will result in `read`, `write`, `recv` and `send` operations
    /// becoming nonblocking, i.e., immediately returning from their calls.
    /// If the IO operation is successful, `Ok` is returned and no further
    /// action is required. If the IO operation could not be completed and needs
    /// to be retried, an error with kind
    /// [`Err(WouldBlock)`](AxError::WouldBlock) is returned.
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) {
        self.nonblock.store(nonblocking, Ordering::Release);
    }

    /// Connects to the given address and port.
    ///
    /// The local port is generated automatically.
    pub async fn connect(&self, remote_addr: IpEndpoint) -> SysResult<()> {
        // 将STATE_CLOSED改为STATE_CONNECTING，在poll_connect的时候，
        // 会再变为STATE_CONNECTED
        self.update_state(STATE_CLOSED, STATE_CONNECTING, || {
            // SAFETY: no other threads can read or write these fields.
            let handle = unsafe { self.handle.get().read() }
                .unwrap_or_else(|| SOCKET_SET.add(SocketSetWrapper::new_tcp_socket()));

            // TODO: check remote addr unreachable
            let bound_endpoint = self.bound_endpoint()?;
            let iface = &ETH0.get().unwrap().iface;
            let (local_endpoint, remote_endpoint) = SOCKET_SET
                .with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                    socket
                        .connect(iface.lock().context(), remote_addr, bound_endpoint)
                        .or_else(|e| match e {
                            // When attempting to perform an operation, the socket is in an
                            // invalid state. Such as attempting to call the connection operation
                            // again on an already connected socket, or performing
                            // the operation on a closed socket
                            ConnectError::InvalidState => {
                                warn!("[TcpSocket::connect] failed: InvalidState");
                                Err(SysError::EBADF)
                            }
                            // The target address or port attempting to connect is unreachable
                            ConnectError::Unaddressable => {
                                warn!("[TcpSocket::connect] failed: Unaddressable");
                                Err(SysError::EADDRNOTAVAIL)
                            }
                        })?;
                    Ok((
                        socket.local_endpoint().unwrap(),
                        socket.remote_endpoint().unwrap(),
                    ))
                })?;
            unsafe {
                // SAFETY: no other threads can read or write these fields as we
                // have changed the state to `BUSY`.
                self.local_addr.get().write(local_endpoint);
                self.peer_addr.get().write(remote_endpoint);
                self.handle.get().write(Some(handle));
            }
            Ok(())
        })
        .unwrap_or_else(|_| {
            warn!("[TcpSocket::connect] failed: already connected");
            Err(SysError::EEXIST)
        })?; // EISCONN

        // Here our state must be `CONNECTING`, and only one thread can run here.
        if self.is_nonblocking() {
            Err(SysError::EINPROGRESS)
        } else {
            self.block_on_async(|| async {
                let NetPollState { writable, .. } = self.poll_connect().await;
                if !writable {
                    warn!("[TcpSocket::connect] failed: try again");
                    Err(SysError::EAGAIN)
                } else if self.get_state() == STATE_CONNECTED {
                    Ok(())
                } else {
                    warn!("[TcpSocket::connect] failed, connection refused");
                    Err(SysError::ECONNREFUSED)
                }
            })
            .await
        }
    }

    /// Binds an unbound socket to the given address and port.
    ///
    /// If the given port is 0, it generates one automatically.
    ///
    /// It's must be called before [`listen`](Self::listen) and
    /// [`accept`](Self::accept).
    pub fn bind(&self, mut local_addr: IpEndpoint) -> SysResult<()> {
        self.update_state(STATE_CLOSED, STATE_CLOSED, || {
            // TODO: check addr is available
            if local_addr.port == 0 {
                let port = get_ephemeral_port()?;
                local_addr.port = port;
                info!("[TcpSocket::bind] local port is 0, use port {port}");
            }
            // SAFETY: no other threads can read or write `self.local_addr` as we
            // have changed the state to `BUSY`.
            unsafe {
                let old = self.local_addr.get().read();
                if old != UNSPECIFIED_ENDPOINT_V4 {
                    warn!("socket bind() failed: {:?} already bound", local_addr);
                    return Err(SysError::EINVAL);
                }
                // FIXME
                if let IpAddress::Ipv6(v6) = local_addr.addr {
                    if v6.is_unspecified() {
                        log::warn!("[TcpSocket::bind] Unstable: just use ipv4 instead of ipv6 when ipv6 is unspecified");
                        local_addr.addr = UNSPECIFIED_IPV4;
                    }
                }
                self.local_addr.get().write(local_addr);
            }
            Ok(())
        })
        .unwrap_or_else(|_| {
            warn!("socket bind() failed: {:?} already bound", local_addr);
            Err(SysError::EINVAL)
        })
    }

    /// Starts listening on the bound address and port.
    ///
    /// It's must be called after [`bind`](Self::bind) and before
    /// [`accept`](Self::accept).
    pub fn listen(&self, waker: &Waker) -> SysResult<()> {
        self.update_state(STATE_CLOSED, STATE_LISTENING, || {
            let bound_endpoint = self.bound_endpoint()?;
            unsafe {
                (*self.local_addr.get()).port = bound_endpoint.port;
            }
            LISTEN_TABLE.listen(bound_endpoint, waker)?;
            info!("[TcpSocket::listen] listening on {bound_endpoint:?}");
            Ok(())
        })
        .unwrap_or(Ok(())) // ignore simultaneous `listen`s.
    }

    /// Accepts a new connection.
    ///
    /// This function will block the calling thread until a new TCP connection
    /// is established. When established, a new [`TcpSocket`] is returned.
    ///
    /// It's must be called after [`bind`](Self::bind) and
    /// [`listen`](Self::listen).
    pub async fn accept(&self) -> SysResult<TcpSocket> {
        if !self.is_listening() {
            warn!("socket accept() failed: not listen");
            return Err(SysError::EINVAL);
        }

        // SAFETY: `self.local_addr` should be initialized after `bind()`.
        let local_port = unsafe { self.local_addr.get().read().port };
        self.block_on(|| {
            let (handle, (local_addr, peer_addr)) = LISTEN_TABLE.accept(local_port)?;
            info!("TCP socket accepted a new connection {}", peer_addr);
            Ok(TcpSocket::new_connected(handle, local_addr, peer_addr))
        })
        .await
    }

    /// Close the connection.
    pub fn shutdown(&self, how: u8) -> SysResult<()> {
        // SAFETY: shutdown won't be called in multiple threads
        unsafe {
            let shutdown = self.shutdown.get();
            match how {
                SHUT_RD => *shutdown |= RCV_SHUTDOWN,
                SHUT_WR => *shutdown |= SEND_SHUTDOWN,
                SHUT_RDWR => *shutdown |= SHUTDOWN_MASK,
                _ => return Err(SysError::EINVAL),
            }
        }

        // stream
        self.update_state(STATE_CONNECTED, STATE_CLOSED, || {
            // SAFETY: `self.handle` should be initialized in a connected socket, and
            // no other threads can read or write it.
            let handle = unsafe { self.handle.get().read().unwrap() };
            SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                warn!(
                    "TCP handle {handle}: shutting down, before state is {:?}",
                    socket.state()
                );
                socket.close();
                warn!(
                    "TCP handle {handle}: shutting down, after state is {:?}",
                    socket.state()
                );
            });
            // unsafe { self.local_addr.get().write(UNSPECIFIED_ENDPOINT) }; // clear bound
            // address
            let timestamp = SOCKET_SET.poll_interfaces();
            SOCKET_SET.check_poll(timestamp);
            Ok(())
        })
        .unwrap_or(Ok(()))?;

        // listener
        self.update_state(STATE_LISTENING, STATE_CLOSED, || {
            // SAFETY: `self.local_addr` should be initialized in a listening socket,
            // and no other threads can read or write it.
            let local_port = unsafe { self.local_addr.get().read().port };
            unsafe { self.local_addr.get().write(UNSPECIFIED_ENDPOINT_V4) }; // clear bound address
            LISTEN_TABLE.unlisten(local_port);
            let timestamp = SOCKET_SET.poll_interfaces();
            SOCKET_SET.check_poll(timestamp);
            Ok(())
        })
        .unwrap_or(Ok(()))?;
        // ignore for other states
        Ok(())
    }

    /// Receives data from the socket, stores it in the given buffer.
    pub async fn recv(&self, buf: &mut [u8]) -> SysResult<usize> {
        let shutdown = unsafe { *self.shutdown.get() };
        if shutdown & RCV_SHUTDOWN != 0 {
            log::warn!("[TcpSocket::recv] shutdown closed read, recv return 0");
            return Ok(0);
        }
        if self.is_connecting() {
            // TODO: 这里是否要加上 waker
            return Err(SysError::EAGAIN);
        } else if !self.is_connected() && shutdown == 0 {
            warn!("socket recv() failed");
            return Err(SysError::ENOTCONN);
        }

        // SAFETY: `self.handle` should be initialized in a connected socket.
        let handle = unsafe { self.handle.get().read().unwrap() };
        let waker = get_waker().await;
        self.block_on(|| {
            SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                log::info!("[TcpSocket::recv] handle{handle} state {} is trying to recv", socket.state());
                if !socket.is_active() {
                    // not open
                    warn!("[TcpSocket::recv] socket recv() failed because handle{handle} is not active");
                    Err(SysError::ECONNREFUSED)
                } else if !socket.may_recv() {
                    // connection closed
                    Ok(0)
                } else if socket.recv_queue() > 0 {
                    // data available
                    // TODO: use socket.recv(|buf| {...})
                    let len = socket.recv_slice(buf).map_err(|_| {
                        warn!("socket recv() failed, badstate");
                        SysError::EBADF
                    })?;
                    Ok(len)
                } else {
                    // no more data
                    log::info!("[TcpSocket::recv] handle{handle} has no data to recv, register waker and suspend");
                    socket.register_recv_waker(&waker);
                    Err(SysError::EAGAIN)
                }
            })
        })
        .await
    }

    /// Transmits data in the given buffer.
    pub async fn send(&self, buf: &[u8]) -> SysResult<usize> {
        let shutdown = unsafe { *self.shutdown.get() };
        if shutdown & SEND_SHUTDOWN != 0 {
            log::warn!("[TcpSocket::send] shutdown closed write, send return 0");
            return Ok(0);
        }
        if self.is_connecting() {
            return Err(SysError::EAGAIN);
        } else if !self.is_connected() && shutdown == 0 {
            warn!("socket send() failed");
            return Err(SysError::ENOTCONN);
        }

        // SAFETY: `self.handle` should be initialized in a connected socket.
        let handle = unsafe { self.handle.get().read().unwrap() };
        let waker = get_waker().await;
        let ret = self.block_on(|| {
            SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                if !socket.is_active() || !socket.may_send() {
                    // closed by remote
                    warn!("socket send() failed, ECONNRESET");
                    Err(SysError::ECONNRESET)
                } else if socket.can_send() {
                    // connected, and the tx buffer is not full
                    // TODO: use socket.send(|buf| {...})
                    let len = socket.send_slice(buf).map_err(|e| {
                        error!("socket recv() failed: bad state, {e:?}");
                        // TODO: Not sure what error should it be
                        SysError::EBADF
                    })?;
                    Ok(len)
                } else {
                    // tx buffer is full
                    log::info!("[TcpSocket::send] handle{handle} send buffer is full, register waker and suspend");
                    socket.register_send_waker(&waker);
                    Err(SysError::EAGAIN)
                }
            })
        })
        .await;
        if let Ok(bytes) = ret {
            if bytes > TCP_TX_BUF_LEN / 2 {
                ksleep_ms(3).await;
            } else {
                yield_now().await;
            }
        }
        SOCKET_SET.poll_interfaces();
        ret
    }

    /// Whether the socket is readable or writable.
    pub async fn poll(&self) -> NetPollState {
        match self.get_state() {
            STATE_CONNECTING => self.poll_connect().await,
            STATE_CONNECTED => self.poll_stream().await,
            STATE_LISTENING => self.poll_listener(),
            STATE_CLOSED => self.poll_closed(),
            _ => NetPollState {
                readable: false,
                writable: false,
                hangup: false,
            },
        }
    }
}

pub struct TcpRecvFuture<'a> {
    socket: &'a TcpSocket,
    buf: &'a [u8],
}

impl<'a> Future for TcpRecvFuture<'a> {
    type Output = SyscallResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        use tcp::State::*;

        let shutdown = unsafe { *self.socket.shutdown.get() };
        if shutdown & RCV_SHUTDOWN != 0 {
            log::warn!("[TcpSocket::recv] shutdown closed read, recv return 0");
            return Poll::Ready(Ok(0));
        }
        if self.socket.is_connecting() {
            // TODO: 这里是否要加上 waker
            log::warn!("[TcpRecvFuture] may loss waker");
            return Poll::Pending;
        } else if !self.socket.is_connected() && shutdown == 0 {
            warn!("socket recv() failed");
            return Poll::Ready(Err(SysError::ENOTCONN));
        }

        // SAFETY: `self.handle` should be initialized in a connected socket.
        let handle = unsafe { self.socket.handle.get().read().unwrap() };
        let ret =SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket|{
            log::info!(
                "[TcpSocket::recv] handle{handle} state {} is trying to recv",
                socket.state()
            );
            if !socket.is_active() {
                // not open
                warn!("[TcpSocket::recv] socket recv() failed because handle{handle} is not active");
                Poll::Ready(Err(SysError::ECONNREFUSED))
            } else if !socket.may_recv() {
                // connection closed
                Poll::Ready(Ok(0))
            } else if socket.recv_queue() > 0 {
                // data available
                // TODO: use socket.recv(|buf| {...})
                // let mut this = self.get_mut();
                // let len = socket.recv_slice(&mut this.buf).map_err(|_| {
                //     warn!("socket recv() failed, badstate");
                //     SysError::EBADF
                // })?;
                // Poll::Ready(Ok(len))
                Poll::Ready(Ok(0))
            } else {
                // no more data
                log::info!(
                    "[TcpSocket::recv] handle{handle} has no data to recv, register waker and suspend"
                );
                if self.socket.is_nonblocking() {
                    return Poll::Ready(Err(SysError::EAGAIN));
                }
                socket.register_recv_waker(cx.waker());
                Poll::Pending
            }
        });
        SOCKET_SET.poll_interfaces();
        ret
    }
}

/// Private methods
impl TcpSocket {
    #[inline]
    fn get_state(&self) -> u8 {
        self.state.load(Ordering::Acquire)
    }

    #[inline]
    fn set_state(&self, state: u8) {
        self.state.store(state, Ordering::Release);
    }

    /// Update the state of the socket atomically.
    ///
    /// If the current state is `expect`, it first changes the state to
    /// `STATE_BUSY`, then calls the given function. If the function returns
    /// `Ok`, it changes the state to `new`, otherwise it changes the state
    /// back to `expect`.
    ///
    /// It returns `Ok` if the current state is `expect`, otherwise it returns
    /// the current state in `Err`.
    fn update_state<F, T>(&self, expect: u8, new: u8, f: F) -> Result<SysResult<T>, u8>
    where
        F: FnOnce() -> SysResult<T>,
    {
        match self
            .state
            .compare_exchange(expect, STATE_BUSY, Ordering::Acquire, Ordering::Acquire)
        {
            Ok(_) => {
                let res = f();
                if res.is_ok() {
                    self.set_state(new);
                } else {
                    self.set_state(expect);
                }
                Ok(res)
            }
            Err(old) => Err(old),
        }
    }

    #[inline]
    fn is_connecting(&self) -> bool {
        self.get_state() == STATE_CONNECTING
    }

    #[inline]
    fn is_connected(&self) -> bool {
        self.get_state() == STATE_CONNECTED
    }

    #[inline]
    fn is_listening(&self) -> bool {
        self.get_state() == STATE_LISTENING
    }

    /// 构建并返回当前对象绑定的网络端点信息。
    /// 具体来说，它从对象的 local_addr
    /// 属性中读取IP地址和端口信息，如果端口未指定则分配一个临时端口，
    /// 并确保返回一个有效的端点（IpListenEndpoint）。
    fn bound_endpoint(&self) -> SysResult<IpListenEndpoint> {
        // SAFETY: no other threads can read or write `self.local_addr`.
        let local_addr = unsafe { self.local_addr.get().read() };
        let port = if local_addr.port != 0 {
            local_addr.port
        } else {
            get_ephemeral_port()?
        };
        assert_ne!(port, 0);
        let addr = if !is_unspecified(local_addr.addr) {
            Some(local_addr.addr)
        } else {
            None
        };
        Ok(IpListenEndpoint { addr, port })
    }

    /// Poll the status of a TCP connection to determine if it has been
    /// established (successful connection) or failed (closed connection)
    ///
    /// Returning `true` indicates that the socket has entered a stable
    /// state(connected or failed) and can proceed to the next step
    async fn poll_connect(&self) -> NetPollState {
        // SAFETY: `self.handle` should be initialized above.
        let handle = unsafe { self.handle.get().read().unwrap() };
        let waker = get_waker().await;
        let writable = SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
            match socket.state() {
                State::SynSent => {
                    // The connection request has been sent but no response
                    socket.register_recv_waker(&waker);
                    false
                }
                // has been received yet
                State::Established => {
                    self.set_state(STATE_CONNECTED); // connected
                    info!(
                        "[TcpSocket::poll_connect] handle {}: connected to {}",
                        handle,
                        socket.remote_endpoint().unwrap(),
                    );
                    true
                }
                _ => {
                    unsafe {
                        self.local_addr.get().write(UNSPECIFIED_ENDPOINT_V4);
                        self.peer_addr.get().write(UNSPECIFIED_ENDPOINT_V4);
                    }
                    self.set_state(STATE_CLOSED); // connection failed
                    true
                }
            }
        });
        NetPollState {
            readable: false,
            writable,
            hangup: false,
        }
    }

    async fn poll_stream(&self) -> NetPollState {
        // SAFETY: `self.handle` should be initialized in a connected socket.
        let handle = unsafe { self.handle.get().read().unwrap() };
        let waker = get_waker().await;
        SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
            // readable 本质上是是否应该继续阻塞，因此为 true 时的条件可以理解为：
            // 1. 套接字已经关闭接收：在这种情况下，即使没有新数据到达，读取操作也不会阻塞，
            //    因为读取会立即返回
            // 2. 套接字中有数据可读：这是最常见的可读情况，表示可以从套接字中读取到数据
            let readable = !socket.may_recv() || socket.can_recv();
            let writable = !socket.may_send() || socket.can_send();
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

    fn poll_listener(&self) -> NetPollState {
        // SAFETY: `self.local_addr` should be initialized in a listening socket.
        let local_addr = unsafe { self.local_addr.get().read() };
        let readable = LISTEN_TABLE.can_accept(local_addr.port);
        NetPollState {
            readable,
            writable: false,
            hangup: false,
        }
    }

    fn poll_closed(&self) -> NetPollState {
        use tcp::State::*;
        let handle = unsafe { self.handle.get().read() };
        if let Some(handle) = handle {
            SOCKET_SET.with_socket_mut::<tcp::Socket, _, _>(handle, |socket| {
                log::warn!(
                    "[TcpSocket::poll_closed] handle {handle} state {}",
                    socket.state()
                );
                let hangup = matches!(socket.state(), CloseWait | FinWait2 | TimeWait);
                NetPollState {
                    readable: false,
                    writable: false,
                    hangup,
                }
            })
        } else {
            NetPollState {
                readable: false,
                writable: false,
                hangup: false,
            }
        }
    }

    /// Block the current thread until the given function completes or fails.
    ///
    /// If the socket is non-blocking, it calls the function once and returns
    /// immediately. Otherwise, it may call the function multiple times if it
    /// returns [`Err(WouldBlock)`](AxError::WouldBlock).
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
                            warn!("[TcpSocket::block_on] has signal");
                            return Err(SysError::EINTR);
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }

    async fn block_on_async<F, T, Fut>(&self, mut f: F) -> SysResult<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = SysResult<T>>,
    {
        if self.is_nonblocking() {
            f().await
        } else {
            loop {
                let timestamp = SOCKET_SET.poll_interfaces();
                let ret = f().await;
                SOCKET_SET.check_poll(timestamp);
                match ret {
                    Ok(t) => return Ok(t),
                    Err(SysError::EAGAIN) => {
                        suspend_now().await;
                        if has_signal() {
                            warn!("[TcpSocket::block_on_async] has signal");
                            return Err(SysError::EINTR);
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }
}

impl Drop for TcpSocket {
    fn drop(&mut self) {
        log::info!("[TcpSocket::Drop] ");
        self.shutdown(SHUT_RDWR).ok();
        // Safe because we have mut reference to `self`.
        if let Some(handle) = unsafe { self.handle.get().read() } {
            SOCKET_SET.remove(handle);
        }
    }
}

fn get_ephemeral_port() -> SysResult<u16> {
    const PORT_START: u16 = 0xc000;
    const PORT_END: u16 = 0xffff;
    static CURR: Mutex<u16> = Mutex::new(PORT_START);

    let mut curr = CURR.lock();
    let mut tries = 0;
    // TODO: more robust
    while tries <= PORT_END - PORT_START {
        let port = *curr;
        if *curr == PORT_END {
            *curr = PORT_START;
        } else {
            *curr += 1;
        }
        if LISTEN_TABLE.can_listen(port) {
            return Ok(port);
        }
        tries += 1;
    }
    warn!("no avaliable ports!");
    Err(SysError::EADDRINUSE)
}
