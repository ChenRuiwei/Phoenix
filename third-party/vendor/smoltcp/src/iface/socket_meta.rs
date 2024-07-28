use super::SocketHandle;
use crate::{
    socket::PollAt,
    time::{Duration, Instant},
    wire::IpAddress,
};

/// Neighbor dependency.
///
/// This enum tracks whether the socket should be polled based on the neighbor
/// it is going to send packets to.
///
/// 跟踪套接字的邻居依赖状态，决定套接字是否可以立即发送数据包或需要等待。
/// 在网络通信中，设备需要知道其邻居（即目标IP地址对应的MAC地址等）才能发送数据包。
/// 通过NeighborState来跟踪邻居的状态，
/// 可以避免在邻居未发现时频繁发送邻居发现请求，从而减少网络流量和资源消耗。
#[derive(Debug, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
enum NeighborState {
    /// Socket can be polled immediately.
    #[default]
    Active,
    /// Socket should not be polled until either `silent_until` passes or
    /// `neighbor` appears in the neighbor cache.
    ///
    /// 套接字需要等待，直到超时时间到达或邻居出现在邻居缓存中
    Waiting {
        neighbor: IpAddress,
        silent_until: Instant,
    },
}

/// Network socket metadata.
///
/// This includes things that only external (to the socket, that is) code
/// is interested in, but which are more conveniently stored inside the socket
/// itself.
///
/// 存储每个套接字的元数据，包括套接字句柄和邻居状态
#[derive(Debug, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct Meta {
    /// Handle of this socket within its enclosing `SocketSet`.
    /// Mainly useful for debug output.
    pub(crate) handle: SocketHandle,
    /// See [NeighborState](struct.NeighborState.html).
    neighbor_state: NeighborState,
}

impl Meta {
    /// Minimum delay between neighbor discovery requests for this particular
    /// socket, in milliseconds.
    ///
    /// See also `iface::NeighborCache::SILENT_TIME`.
    ///
    /// 邻居发现请求的最小间隔时间，防止在短时间内频繁发送发现请求
    pub(crate) const DISCOVERY_SILENT_TIME: Duration = Duration::from_millis(1_000);

    /// poll_at方法根据邻居状态和自定义函数（has_neighbor）决定套接字的轮询时间，可以动态调整轮询策略
    pub(crate) fn poll_at<F>(&self, socket_poll_at: PollAt, has_neighbor: F) -> PollAt
    where
        F: Fn(IpAddress) -> bool,
    {
        match self.neighbor_state {
            NeighborState::Active => socket_poll_at,
            NeighborState::Waiting { neighbor, .. } if has_neighbor(neighbor) => socket_poll_at,
            NeighborState::Waiting { silent_until, .. } => PollAt::Time(silent_until),
        }
    }

    pub(crate) fn egress_permitted<F>(&mut self, timestamp: Instant, has_neighbor: F) -> bool
    where
        F: Fn(IpAddress) -> bool,
    {
        match self.neighbor_state {
            NeighborState::Active => true,
            NeighborState::Waiting {
                neighbor,
                silent_until,
            } => {
                if has_neighbor(neighbor) {
                    net_trace!(
                        "{}: neighbor {} discovered, unsilencing",
                        self.handle,
                        neighbor
                    );
                    self.neighbor_state = NeighborState::Active;
                    true
                } else if timestamp >= silent_until {
                    net_trace!(
                        "{}: neighbor {} silence timer expired, rediscovering",
                        self.handle,
                        neighbor
                    );
                    true
                } else {
                    false
                }
            }
        }
    }

    pub(crate) fn neighbor_missing(&mut self, timestamp: Instant, neighbor: IpAddress) {
        net_trace!(
            "{}: neighbor {} missing, silencing until t+{}",
            self.handle,
            neighbor,
            Self::DISCOVERY_SILENT_TIME
        );
        self.neighbor_state = NeighborState::Waiting {
            neighbor,
            silent_until: timestamp + Self::DISCOVERY_SILENT_TIME,
        };
    }
}
