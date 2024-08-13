//! Adapted from ArceOS

#![no_std]
#![no_main]
#![feature(new_uninit)]

extern crate alloc;
use alloc::{boxed::Box, vec, vec::Vec};
use core::{cell::RefCell, future::Future, ops::DerefMut, panic, time::Duration};

use arch::time::{get_time_duration, get_time_us};
use crate_interface::call_interface;
use device_core::{error::DevError, NetBufPtrOps, NetDevice};
use listen_table::*;
use log::*;
pub use smoltcp::wire::{IpAddress, IpEndpoint, IpListenEndpoint, Ipv4Address, Ipv6Address};
pub(crate) use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet},
    phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
    socket::{self, AnySocket},
    time::{Duration as SmolDuration, Instant as SmolInstant},
    wire::{EthernetAddress, HardwareAddress, IpCidr},
};
use spin::{Lazy, Once};
use sync::mutex::SpinNoIrqLock;
use timer::{Timer, TimerEvent, TIMER_MANAGER};
pub mod addr;
pub mod bench;
pub mod listen_table;
pub mod portmap;
pub mod tcp;
pub mod udp;

pub(crate) type Mutex<T> = SpinNoIrqLock<T>;

macro_rules! env_or_default {
    ($key:literal) => {
        match option_env!($key) {
            Some(val) => val,
            None => "",
        }
    };
}

/// Defined in makefile
const IP: &str = env_or_default!("Phoenix_IP");
const GATEWAY: &str = env_or_default!("Phoenix_GW");
const DNS_SEVER: &str = "8.8.8.8";
const IP_PREFIX: u8 = 24;

const STANDARD_MTU: usize = 1500;

const RANDOM_SEED: u64 = 0xA2CE_05A2_CE05_A2CE;

pub const TCP_RX_BUF_LEN: usize = 64 * 1024;
pub const TCP_TX_BUF_LEN: usize = 64 * 1024;
const UDP_RX_BUF_LEN: usize = 64 * 1024;
const UDP_TX_BUF_LEN: usize = 64 * 1024;
const LISTEN_QUEUE_SIZE: usize = 512;

static LISTEN_TABLE: Lazy<ListenTable> = Lazy::new(ListenTable::new);
static SOCKET_SET: Lazy<SocketSetWrapper> = Lazy::new(SocketSetWrapper::new);
static ETH0: Once<InterfaceWrapper> = Once::new();

/// SocketSet is a collection of sockets that contain multiple different types
/// of sockets (such as TCP, UDP, ICMP, etc.). It provides a mechanism to manage
/// and operate these sockets, including polling socket status, processing data
/// transmission and reception, etc. It is similar to `FdTable` and
/// `SocketHandle` is similar to `fd`
struct SocketSetWrapper<'a>(Mutex<SocketSet<'a>>);

/// A wrapper for network devices, providing interior mutability for
/// `NetDevice`.
struct DeviceWrapper {
    /// The inner network device wrapped in a `RefCell` for interior mutability.
    inner: RefCell<Box<dyn NetDevice>>,
}

/// A wrapper for network interfaces, containing device and interface details
/// and providing thread-safe access via `Mutex`.
struct InterfaceWrapper {
    /// The name of the network interface.
    name: &'static str,
    /// The Ethernet address of the network interface.
    ether_addr: EthernetAddress,
    /// The device wrapper protected by a `Mutex` to ensure thread-safe access.
    dev: Mutex<DeviceWrapper>,
    /// The network interface protected by a `Mutex` to ensure thread-safe
    /// access.
    iface: Mutex<Interface>,
}

impl<'a> SocketSetWrapper<'a> {
    fn new() -> Self {
        Self(Mutex::new(SocketSet::new(vec![])))
    }

    /// return a `tcp::Socket` defined in `smoltcp`
    pub fn new_tcp_socket() -> socket::tcp::Socket<'a> {
        let tcp_rx_buffer = socket::tcp::SocketBuffer::new(vec![0; TCP_RX_BUF_LEN]);
        let tcp_tx_buffer = socket::tcp::SocketBuffer::new(vec![0; TCP_TX_BUF_LEN]);
        socket::tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer)
    }

    /// return a `udp::Socket` defined in `smoltcp`
    pub fn new_udp_socket() -> socket::udp::Socket<'a> {
        let udp_rx_buffer = socket::udp::PacketBuffer::new(
            vec![socket::udp::PacketMetadata::EMPTY; 8],
            vec![0; UDP_RX_BUF_LEN],
        );
        let udp_tx_buffer = socket::udp::PacketBuffer::new(
            vec![socket::udp::PacketMetadata::EMPTY; 8],
            vec![0; UDP_TX_BUF_LEN],
        );
        socket::udp::Socket::new(udp_rx_buffer, udp_tx_buffer)
    }

    #[allow(dead_code)]
    pub fn new_dns_socket() -> socket::dns::Socket<'a> {
        let server_addr = DNS_SEVER.parse().expect("invalid DNS server address");
        socket::dns::Socket::new(&[server_addr], vec![])
    }

    /// return `SocketHandle`, which is Similar to file descriptors in the
    /// operating system
    pub fn add<T: AnySocket<'a>>(&self, socket: T) -> SocketHandle {
        let handle = self.0.lock().add(socket);
        debug!("[net::SocketSetWrapper] sockethandle {}: created", handle);
        handle
    }

    pub fn with_socket<T: AnySocket<'a>, R, F>(&self, handle: SocketHandle, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        let set = self.0.lock();
        let socket = set.get(handle);
        f(socket)
    }

    #[allow(dead_code)]
    pub async fn with_socket_async<T: AnySocket<'a>, R, F, Fut>(
        &self,
        handle: SocketHandle,
        f: F,
    ) -> R
    where
        F: FnOnce(&T) -> Fut,
        Fut: Future<Output = R>,
    {
        let set = self.0.lock();
        let socket = set.get(handle);
        f(socket).await
    }

    #[allow(dead_code)]
    pub async fn with_socket_mut_async<T: AnySocket<'a>, R, F, Fut>(
        &self,
        handle: SocketHandle,
        f: F,
    ) -> R
    where
        F: FnOnce(&mut T) -> Fut,
        Fut: Future<Output = R>,
    {
        let mut set = self.0.lock();
        let socket = set.get_mut(handle);
        f(socket).await
    }

    pub fn with_socket_mut<T: AnySocket<'a>, R, F>(&self, handle: SocketHandle, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut set = self.0.lock();
        let socket = set.get_mut(handle);
        f(socket)
    }

    pub fn poll_interfaces(&self) -> smoltcp::time::Instant {
        ETH0.get().unwrap().poll(&self.0)
    }

    pub fn check_poll(&self, timestamp: SmolInstant) {
        ETH0.get().unwrap().check_poll(timestamp, &self.0)
    }

    pub fn remove(&self, handle: SocketHandle) {
        self.0.lock().remove(handle);
        debug!("socket {}: destroyed", handle);
    }
}

impl InterfaceWrapper {
    fn new(name: &'static str, dev: Box<dyn NetDevice>, ether_addr: EthernetAddress) -> Self {
        // let mut config = Config::new(HardwareAddress::Ethernet(ether_addr));
        // let mut config = if ether_addr == EthernetAddress([0, 0, 0, 0, 0, 0]) {
        //     log::error!("[InterfaceWrapper] use HardwareAddress::Ip");
        //     Config::new(HardwareAddress::Ip)
        // } else {
        //     Config::new(HardwareAddress::Ethernet(ether_addr))
        // };
        let mut config = match dev.capabilities().medium {
            Medium::Ethernet => Config::new(HardwareAddress::Ethernet(ether_addr)),
            Medium::Ip => Config::new(HardwareAddress::Ip),
        };
        config.random_seed = RANDOM_SEED;

        let mut dev = DeviceWrapper::new(dev);
        let iface = Mutex::new(Interface::new(config, &mut dev, Self::current_time()));
        Self {
            name,
            ether_addr,
            dev: Mutex::new(dev),
            iface,
        }
    }

    fn current_time() -> SmolInstant {
        SmolInstant::from_micros_const(get_time_us() as i64)
    }

    fn ins_to_duration(instant: SmolInstant) -> Duration {
        Duration::from_micros(instant.total_micros() as u64)
    }

    fn dur_to_duration(duration: SmolDuration) -> Duration {
        Duration::from_micros(duration.total_micros() as u64)
    }

    pub fn name(&self) -> &str {
        self.name
    }

    pub fn ethernet_address(&self) -> EthernetAddress {
        self.ether_addr
    }

    pub fn setup_ip_addr(&self, ips: Vec<IpCidr>) {
        let mut iface = self.iface.lock();
        iface.update_ip_addrs(|ip_addrs| ip_addrs.extend(ips));
    }

    pub fn setup_gateway(&self, gateway: IpAddress) {
        let mut iface = self.iface.lock();
        match gateway {
            IpAddress::Ipv4(v4) => iface.routes_mut().add_default_ipv4_route(v4).unwrap(),
            IpAddress::Ipv6(_) => unimplemented!(),
        };
    }

    /// handling the sending and receiving of network packets and updating the
    /// protocol stack status.
    ///
    /// return what time it should poll next
    pub fn poll(&self, sockets: &Mutex<SocketSet>) -> SmolInstant {
        let mut dev = self.dev.lock();
        let mut iface = self.iface.lock();
        let mut sockets = sockets.lock();
        let timestamp = Self::current_time();
        let result = iface.poll(timestamp, dev.deref_mut(), &mut sockets);
        log::warn!("[net::InterfaceWrapper::poll] does something have been changed? {result:?}");
        timestamp
    }

    // pub fn poll_at(&self, sockets: &Mutex<SocketSet>) {
    //     let mut iface = self.iface.lock();
    //     let mut sockets = sockets.lock();
    //     let timestamp = Self::current_time();
    //     if let Some(next_poll_time) = iface
    //         .poll_at(timestamp, &mut sockets)
    //         .map(Self::to_duration)
    //     {
    //         if next_poll_time.is_zero() || next_poll_time <= get_time_duration()
    // {             iface.poll(
    //                 Self::current_time(),
    //                 self.dev.lock().deref_mut(),
    //                 &mut sockets,
    //             );
    //             error!("poll");
    //         } else {
    //             error!(
    //                 "add timer expired {}, now {}",
    //                 next_poll_time.as_micros(),
    //                 get_time_duration().as_micros(),
    //             );
    //             let timer = Timer::new(next_poll_time, Box::new(PollTimer {}));
    //             TIMER_MANAGER.add_timer(timer);
    //         }
    //     } else {
    //         error!("don't need to poll");
    //     }
    // }

    pub fn check_poll(&self, timestamp: SmolInstant, sockets: &Mutex<SocketSet>) {
        let mut iface = self.iface.lock();
        let mut sockets = sockets.lock();
        match iface
            .poll_delay(timestamp, &mut sockets)
            .map(Self::dur_to_duration)
        {
            Some(Duration::ZERO) => {
                iface.poll(
                    Self::current_time(),
                    self.dev.lock().deref_mut(),
                    &mut sockets,
                );
            }
            Some(delay) => {
                let next_poll = delay + Self::ins_to_duration(timestamp);
                let current = get_time_duration();
                if next_poll < current {
                    iface.poll(
                        Self::current_time(),
                        self.dev.lock().deref_mut(),
                        &mut sockets,
                    );
                } else {
                    let timer = Timer::new(next_poll, Box::new(PollTimer {}));
                    TIMER_MANAGER.add_timer(timer);
                }
            }
            None => {
                let timer = Timer::new(
                    get_time_duration() + Duration::from_millis(2),
                    Box::new(PollTimer {}),
                );
                TIMER_MANAGER.add_timer(timer);
            }
        }
    }

    // pub fn auto_poll(&self, sockets: &Mutex<SocketSet>) {
    //     if let Some(next_poll) = self.poll_at(sockets) {
    //         log::debug!(
    //             "current time is {:?}, we should poll next time is {}",
    //             get_time_duration().as_micros(),
    //             next_poll.as_micros()
    //         );

    //     }
    // }
}

pub fn check_poll(timestamp: SmolInstant) {
    SOCKET_SET.check_poll(timestamp)
}

/// Poll the network stack.
///
/// It may receive packets from the NIC and process them, and transmit queued
/// packets to the NIC.
pub fn poll_interfaces() -> smoltcp::time::Instant {
    SOCKET_SET.poll_interfaces()
}

// pub fn auto_poll_interfaces() {
//     SOCKET_SET.auto_poll_interfaces()
// }

struct PollTimer;

impl TimerEvent for PollTimer {
    fn callback(self: Box<Self>) -> Option<Timer> {
        poll_interfaces();
        None
    }
}

impl DeviceWrapper {
    fn new(inner: Box<dyn NetDevice>) -> Self {
        Self {
            inner: RefCell::new(inner),
        }
    }
}

impl Device for DeviceWrapper {
    type RxToken<'a> = NetRxToken<'a> where Self: 'a;
    type TxToken<'a> = NetTxToken<'a> where Self: 'a;

    fn receive(
        &mut self,
        _timestamp: smoltcp::time::Instant,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let mut dev = self.inner.borrow_mut();
        if let Err(e) = dev.recycle_tx_buffers() {
            warn!("recycle_tx_buffers failed: {:?}", e);
            return None;
        }

        if !dev.can_transmit() {
            return None;
        }
        let rx_buf = match dev.receive() {
            Ok(buf) => buf,
            Err(err) => {
                if !matches!(err, DevError::Again) {
                    warn!("receive failed: {:?}", err);
                }
                return None;
            }
        };
        Some((NetRxToken(&self.inner, rx_buf), NetTxToken(&self.inner)))
    }

    fn transmit(&mut self, _timestamp: smoltcp::time::Instant) -> Option<Self::TxToken<'_>> {
        let mut dev = self.inner.borrow_mut();
        if let Err(e) = dev.recycle_tx_buffers() {
            warn!("recycle_tx_buffers failed: {:?}", e);
            return None;
        }
        if dev.can_transmit() {
            Some(NetTxToken(&self.inner))
        } else {
            None
        }
    }

    fn capabilities(&self) -> DeviceCapabilities {
        self.inner.borrow().capabilities()
    }
}

struct NetRxToken<'a>(&'a RefCell<Box<dyn NetDevice>>, Box<dyn NetBufPtrOps>);
struct NetTxToken<'a>(&'a RefCell<Box<dyn NetDevice>>);

impl<'a> RxToken for NetRxToken<'a> {
    fn preprocess(&self, sockets: &mut SocketSet<'_>) {
        let medium = self.0.borrow().capabilities().medium;
        let is_ethernet = medium == Medium::Ethernet;
        snoop_tcp_packet(self.1.packet(), sockets, is_ethernet).ok();
    }

    /// 此方法接收数据包，然后以原始数据包字节作为参数调用给定的闭包f。
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut rx_buf = self.1;
        debug!(
            "[RxToken::consume] RECV {} bytes",
            rx_buf.packet_len(),
            // rx_buf.packet()
        );
        let result = f(rx_buf.packet_mut());
        self.0.borrow_mut().recycle_rx_buffer(rx_buf).unwrap();
        result
    }
}

impl<'a> TxToken for NetTxToken<'a> {
    /// 构造一个大小为len的传输缓冲区，
    /// 并使用对该缓冲区的可变引用调用传递的闭包f。
    /// 闭包应在缓冲区中构造一个有效的网络数据包（例如以太网数据包）。
    /// 当闭包返回时，传输缓冲区被发送出去。
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut dev = self.0.borrow_mut();
        let mut tx_buf = dev.alloc_tx_buffer(len).unwrap();
        let ret = f(tx_buf.packet_mut());
        debug!(
            "[TxToken::consume] SEND {} bytes",
            len,
            // tx_buf.packet()
        );
        dev.transmit(tx_buf).unwrap();
        ret
    }
}

fn snoop_tcp_packet(
    buf: &[u8],
    sockets: &mut SocketSet<'_>,
    is_ethernet: bool,
) -> Result<(), smoltcp::wire::Error> {
    use smoltcp::wire::{EthernetFrame, IpProtocol, Ipv4Packet, TcpPacket};

    // let ether_frame = EthernetFrame::new_checked(buf)?;
    // let ipv4_packet = Ipv4Packet::new_checked(ether_frame.payload())?;
    let ipv4_packet = if is_ethernet {
        let ether_frame = EthernetFrame::new_checked(buf)?;
        Ipv4Packet::new_checked(ether_frame.payload())?
    } else {
        Ipv4Packet::new_checked(buf)?
    };
    if ipv4_packet.next_header() == IpProtocol::Tcp {
        let tcp_packet = TcpPacket::new_checked(ipv4_packet.payload())?;
        let src_addr = (ipv4_packet.src_addr(), tcp_packet.src_port()).into();
        let dst_addr = (ipv4_packet.dst_addr(), tcp_packet.dst_port()).into();
        let is_first = tcp_packet.syn() && !tcp_packet.ack();
        if is_first {
            // create a socket for the first incoming TCP packet, as the later accept()
            // returns.
            info!("[snoop_tcp_packet] receive TCP");
            LISTEN_TABLE.incoming_tcp_packet(src_addr, dst_addr, sockets);
        }
    }
    Ok(())
}

/// net poll results.
#[derive(Debug, Default, Clone, Copy)]
pub struct NetPollState {
    /// Object can be read now.
    pub readable: bool,
    /// Object can be writen now.
    pub writable: bool,
    pub hangup: bool,
}

/// Benchmark raw socket transmit bandwidth.
pub fn bench_transmit() {
    ETH0.get().unwrap().dev.lock().bench_transmit_bandwidth();
}

/// Benchmark raw socket receive bandwidth.
pub fn bench_receive() {
    ETH0.get().unwrap().dev.lock().bench_receive_bandwidth();
}

#[crate_interface::def_interface]
pub trait HasSignalIf: Send + Sync {
    fn has_signal() -> bool;
}

pub(crate) fn has_signal() -> bool {
    call_interface!(HasSignalIf::has_signal())
}

// 下面是来自系统调用的how flag
pub const SHUT_RD: u8 = 0;
pub const SHUT_WR: u8 = 1;
pub const SHUT_RDWR: u8 = 2;

/// 表示读方向已关闭（相当于SHUT_RD）
pub const RCV_SHUTDOWN: u8 = 1;
/// 表示写方向已关闭（相当于SHUT_WR）
pub const SEND_SHUTDOWN: u8 = 2;
/// 表示读和写方向都已关闭（相当于SHUT_RDWR）
pub const SHUTDOWN_MASK: u8 = 3;

pub fn init_network(net_dev: Box<dyn NetDevice>, is_loopback: bool) {
    info!("Initialize network subsystem...");
    let ether_addr = EthernetAddress(net_dev.mac_address().0);
    let eth0 = InterfaceWrapper::new("eth0", net_dev, ether_addr);

    // let ip = IP.parse().expect("invalid IP address");

    let gateway = GATEWAY.parse().expect("invalid gateway IP address");
    let ip;
    let ip_addrs = if is_loopback {
        ip = "127.0.0.1".parse().unwrap();
        vec![IpCidr::new(ip, 8)]
    } else {
        ip = IP.parse().expect("invalid IP address");
        vec![
            IpCidr::new("127.0.0.1".parse().unwrap(), 8),
            IpCidr::new(ip, IP_PREFIX),
        ]
    };
    eth0.setup_ip_addr(ip_addrs);
    eth0.setup_gateway(gateway);

    ETH0.call_once(|| eth0);

    info!("created net interface {:?}:", ETH0.get().unwrap().name());
    info!("  ether:    {}", ETH0.get().unwrap().ethernet_address());
    info!("  ip:       {}/{}", ip, IP_PREFIX);
    info!("  gateway:  {}", gateway);
}
