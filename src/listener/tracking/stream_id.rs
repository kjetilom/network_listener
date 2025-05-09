use std::fmt::{self, Display};
use std::hash::Hash;
use std::net::IpAddr;

use crate::{Direction, ParsedPacket, TransportPacket};
use pnet::packet::ip::IpNextHeaderProtocol;
use procfs::net::{TcpNetEntry, UdpNetEntry};

use crate::probe::iperf_json::Connected;

pub type IpPair = Pair<IpAddr>;

/// Marker trait for types that can be used in a `Pair`.
///
/// Abstraction layer to simplify the logic behind the 5-tuple and 3-tuple keys.
pub trait Pairable: PartialEq + Eq + Hash + Clone + Copy {}
impl<T: PartialEq + Eq + Hash + Clone + Copy> Pairable for T {}
impl<T: Pairable> Eq for Pair<T> {}

#[derive(Debug, Hash, Clone, Copy)]
pub struct Pair<T: Pairable> {
    local: T,
    remote: T,
}

/// An ordered or unordered pair of values (e.g., IP addresses or ports).
/// A single instance of the tool will never have two stream trackers with the
/// same 5-tuple or 3-tuple, so we want to match incoming and outgoing packets
/// to the same tracker.
///
/// Invariants:
/// - `Pair::new(a, b)` keeps `local = a`, `remote = b`.
/// - Equality is symmetric: `(a, b) == (b, a)`.
impl<T: Pairable> Pair<T> {
    pub fn new(local: T, remote: T) -> Self {
        Pair { local, remote }
    }

    /// Build a [`Pair`] from source and destination values based on a packet `direction`.
    ///
    /// - For `Incoming`, the guest sees `t_dst` as local and `t_src` as remote.
    /// - For `Outgoing`, `t_src` is local and `t_dst` is remote.
    pub fn from_direction(t_src: T, t_dst: T, direction: Direction) -> Self {
        match direction {
            Direction::Incoming => Self::new(t_dst, t_src),
            Direction::Outgoing => Self::new(t_src, t_dst),
        }
    }

    /// Returns the "local" side of the pair.
    pub fn local(&self) -> T {
        self.local
    }

    /// Returns the "remote" side of the pair.
    pub fn remote(&self) -> T {
        self.remote
    }
}

impl<T: Pairable> PartialEq<Pair<T>> for Pair<T> {
    /// Two [`Pair`]s are equal if they contain the same two elements, regardless of order.
    fn eq(&self, other: &Self) -> bool {
        self.local == other.local && self.remote == other.remote
            || self.local == other.remote && self.remote == other.local
    }
}

impl Pair<IpAddr> {
    /// Extract a `Pair<IpAddr>` from a parsed packet, ordering by direction.
    pub fn from_packet(packet: &ParsedPacket) -> Self {
        Pair::from_direction(packet.src_ip, packet.dst_ip, packet.direction)
    }
}

impl Display for Pair<IpAddr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.local, self.remote)
    }
}

/// A key identifying a transport-layer stream: a pair of ports plus protocol.
///
/// The [`StreamKey`] is used inside the `StreamManager` to identify the stream.
#[derive(Debug, PartialEq, Hash, Eq)]
pub struct StreamKey {
    ports: Pair<Option<u16>>,
    protocol: IpNextHeaderProtocol,
}



impl StreamKey {
    pub fn new(protocol: IpNextHeaderProtocol, local: Option<u16>, remote: Option<u16>) -> Self {
        StreamKey {
            ports: Pair::new(local, remote),
            protocol,
        }
    }

    /// Construct from separate src/dst ports and direction.
    pub fn from_direction(
        protocol: IpNextHeaderProtocol,
        src: Option<u16>,
        dst: Option<u16>,
        direction: Direction,
    ) -> Self {
        let ports = Pair::from_direction(src, dst, direction);
        StreamKey { ports, protocol }
    }

    /// Derive a [`StreamKey`] from a parsed packet's transport layer.
    ///
    /// Supports TCP and UDP; uses `None` ports otherwise.
    pub fn from_packet(packet: &ParsedPacket) -> Self {
        match &packet.transport {
            TransportPacket::TCP {
                src_port, dst_port, ..
            }
            | TransportPacket::UDP {
                src_port, dst_port, ..
            } => StreamKey::from_direction(
                packet.transport.get_ip_proto(),
                Some(*src_port),
                Some(*dst_port),
                packet.direction,
            ),
            _ => StreamKey::new(packet.transport.get_ip_proto(), None, None),
        }
    }
}

impl Display for StreamKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.protocol)
    }
}


/// Create a `StreamKey` and `Pair<IpAddr>` from an `iperf` JSON-connected struct.
pub fn from_iperf_connected(
    connected: &Connected,
    proto: IpNextHeaderProtocol,
) -> (StreamKey, Pair<IpAddr>) {
    (
        StreamKey::new(
            proto,
            Some(connected.local_port as u16),
            Some(connected.remote_port as u16),
        ),
        Pair::new(
            connected.local_host.parse().unwrap(),
            connected.remote_host.parse().unwrap(),
        ),
    )
}

/// Macro to generate helpers from procfs net entries (TCP/UDP).
///
/// Each function returns the corresponding `StreamKey` and IP `Pair`.
macro_rules! from_net_entry {
    ($func_name:ident, $net_type:ty) => {
        pub fn $func_name(
            entry: &$net_type,
            protocol: IpNextHeaderProtocol,
        ) -> (StreamKey, Pair<IpAddr>) {
            (
                StreamKey::new(
                    protocol,
                    Some(entry.local_address.port()),
                    Some(entry.remote_address.port()),
                ),
                Pair::new(entry.local_address.ip(), entry.remote_address.ip()),
            )
        }
    };
}

from_net_entry!(from_tcp_net_entry, TcpNetEntry);
from_net_entry!(from_udp_net_entry, UdpNetEntry);

#[cfg(test)]
mod tests {
    use pnet::packet::ip::IpNextHeaderProtocols;
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;

    #[test]
    fn test_pair() {
        let pair = Pair::new(1, 2);
        assert_eq!(pair.local(), 1);
        assert_eq!(pair.remote(), 2);
    }

    #[test]
    fn test_pair_from_direction() {
        let pair = Pair::from_direction(1, 2, Direction::Incoming);
        assert_eq!(pair.local(), 2);
        assert_eq!(pair.remote(), 1);
        let pair = Pair::from_direction(1, 2, Direction::Outgoing);
        assert_eq!(pair.local(), 1);
        assert_eq!(pair.remote(), 2);
    }

    #[test]
    fn test_pair_eq() {
        let pair1 = Pair::new(1, 2);
        let pair2 = Pair::new(2, 1);
        assert_eq!(pair1, pair2);
        assert_eq!(pair1, pair1);
    }

    #[test]
    fn test_stream_key() {
        let key = StreamKey::new(IpNextHeaderProtocols::Tcp, Some(1), Some(2));
        assert_eq!(key.ports.local(), Some(1));
        assert_eq!(key.ports.remote(), Some(2));
        assert_eq!(key.protocol, IpNextHeaderProtocols::Tcp);
    }

    #[test]
    fn test_stream_key_from_direction() {
        let key = StreamKey::from_direction(
            IpNextHeaderProtocols::Tcp,
            Some(1),
            Some(2),
            Direction::Incoming,
        );
        assert_eq!(key.ports.local(), Some(2));
        assert_eq!(key.ports.remote(), Some(1));
        assert_eq!(key.protocol, IpNextHeaderProtocols::Tcp);
    }

    #[test]
    fn test_ip_pair() {
        let pair = Pair::new(
            IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
            IpAddr::V4(Ipv4Addr::new(5, 6, 7, 8)),
        );
        assert_eq!(pair.local(), IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)));
        assert_eq!(pair.remote(), IpAddr::V4(Ipv4Addr::new(5, 6, 7, 8)));
    }

    #[test]
    fn test_asymmetric_ip_pair_eq() {
        let pair1 = Pair::new(
            IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
            IpAddr::V4(Ipv4Addr::new(5, 6, 7, 8)),
        );
        let pair2 = Pair::new(
            IpAddr::V4(Ipv4Addr::new(5, 6, 7, 8)),
            IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
        );
        assert_eq!(pair1, pair2);
    }

    #[test]
    fn test_stream_key_asymmetric() {
        let key1 = StreamKey::new(IpNextHeaderProtocols::Tcp, Some(1), Some(2));
        let key2 = StreamKey::new(IpNextHeaderProtocols::Tcp, Some(2), Some(1));
        assert_eq!(key1, key2);

        let key3 = StreamKey::new(IpNextHeaderProtocols::Udp, Some(1), Some(2));
        assert_ne!(key1, key3);
    }
}
