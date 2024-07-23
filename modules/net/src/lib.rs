#![no_std]
#![no_main]
#![feature(new_uninit)]
extern crate alloc;
use alloc::{boxed::Box, vec, vec::Vec};
use core::{cell::RefCell, future::Future, ops::DerefMut, panic};

use arch::time::get_time_us;
use device_core::{error::DevError, NetBufPtrOps, NetDriverOps};
use listen_table::*;
use log::*;
pub use smoltcp::wire::{IpAddress, IpEndpoint, IpListenEndpoint, Ipv4Address, Ipv6Address};
use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet},
    phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
    socket::{self, AnySocket},
    time::Instant,
    wire::{EthernetAddress, HardwareAddress, IpCidr},
};
use spin::{Lazy, Once};
use sync::mutex::SpinNoIrqLock;
pub mod addr;
pub mod bench;
pub mod listen_table;
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

const TCP_RX_BUF_LEN: usize = 64 * 1024;
const TCP_TX_BUF_LEN: usize = 64 * 1024;
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

struct DeviceWrapper {
    inner: RefCell<Box<dyn NetDriverOps>>, /* use `RefCell` is enough since it's wrapped in
                                            * `Mutex` in
                                            * `InterfaceWrapper`. */
}

struct InterfaceWrapper {
    name: &'static str,
    ether_addr: EthernetAddress,
    dev: Mutex<DeviceWrapper>,
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

    pub fn poll_interfaces(&self) {
        ETH0.get().unwrap().poll(&self.0);
    }

    pub fn remove(&self, handle: SocketHandle) {
        self.0.lock().remove(handle);
        debug!("socket {}: destroyed", handle);
    }
}

impl InterfaceWrapper {
    fn new(name: &'static str, dev: Box<dyn NetDriverOps>, ether_addr: EthernetAddress) -> Self {
        // let mut config = Config::new(HardwareAddress::Ethernet(ether_addr));
        // let mut config = if ether_addr == EthernetAddress([0, 0, 0, 0, 0, 0]) {
        //     log::error!("[InterfaceWrapper] use HardwareAddress::Ip");
        //     Config::new(HardwareAddress::Ip)
        // } else {
        //     Config::new(HardwareAddress::Ethernet(ether_addr))
        // };
        let mut config = match dev.medium() {
            Medium::Ethernet => Config::new(HardwareAddress::Ethernet(ether_addr)),
            Medium::Ip => Config::new(HardwareAddress::Ip),
            _ => panic!(),
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

    fn current_time() -> Instant {
        Instant::from_micros_const(get_time_us() as i64)
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
    pub fn poll(&self, sockets: &Mutex<SocketSet>) {
        let mut dev = self.dev.lock();
        let mut iface = self.iface.lock();
        let mut sockets = sockets.lock();
        let timestamp = Self::current_time();
        let result = iface.poll(timestamp, dev.deref_mut(), &mut sockets);
        log::warn!("[net::poll] does something have been changed? {result:?}")
    }
}

impl DeviceWrapper {
    fn new(inner: Box<dyn NetDriverOps>) -> Self {
        Self {
            inner: RefCell::new(inner),
        }
    }
}

impl Device for DeviceWrapper {
    type RxToken<'a> = NetRxToken<'a> where Self: 'a;
    type TxToken<'a> = NetTxToken<'a> where Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
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

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
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
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1514;
        caps.max_burst_size = None;
        caps.medium = self.inner.borrow().medium();
        caps
    }
}

struct NetRxToken<'a>(&'a RefCell<Box<dyn NetDriverOps>>, Box<dyn NetBufPtrOps>);
struct NetTxToken<'a>(&'a RefCell<Box<dyn NetDriverOps>>);

impl<'a> RxToken for NetRxToken<'a> {
    fn preprocess(&self, sockets: &mut SocketSet<'_>) {
        let medium = self.0.borrow().medium();
        let is_ethernet = medium == Medium::Ethernet;
        snoop_tcp_packet(self.1.packet(), sockets, is_ethernet).ok();
    }

    /// 此方法接收数据包，然后以原始数据包字节作为参数调用给定的闭包f。
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut rx_buf = self.1;
        warn!(
            "[RxToken::consume] RECV {} bytes: {:02X?}",
            rx_buf.packet_len(),
            rx_buf.packet()
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
        warn!(
            "[TxToken::consume] SEND {} bytes: {:02X?}",
            len,
            tx_buf.packet()
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
            error!("[snoop_tcp_packet] receive TCP");
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
}

/// Poll the network stack.
///
/// It may receive packets from the NIC and process them, and transmit queued
/// packets to the NIC.
pub fn poll_interfaces() {
    SOCKET_SET.poll_interfaces();
}

/// Benchmark raw socket transmit bandwidth.
pub fn bench_transmit() {
    ETH0.get().unwrap().dev.lock().bench_transmit_bandwidth();
}

/// Benchmark raw socket receive bandwidth.
pub fn bench_receive() {
    ETH0.get().unwrap().dev.lock().bench_receive_bandwidth();
}

pub fn init_network(net_dev: Box<dyn NetDriverOps>, is_loopback: bool) {
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
