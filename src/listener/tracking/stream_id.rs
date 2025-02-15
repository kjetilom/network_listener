use std::fmt::{self, Display};
use std::hash::Hash;
use std::net::IpAddr;
use std::ops::{Deref, DerefMut};

use pnet::packet::ip::IpNextHeaderProtocol;
use procfs::net::{TcpNetEntry, UdpNetEntry};

use crate::probe::iperf_json::Connected;

use super::super::packet::ParsedPacket;
use super::super::packet::TransportPacket;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub struct IpPair {
    pair: (IpAddr, IpAddr),
}

enum TaggedIpAddr {
    Local(IpAddr),
    Remote(IpAddr),
}

impl Deref for TaggedIpAddr {
    type Target = IpAddr;

    fn deref(&self) -> &Self::Target {
        match self {
            TaggedIpAddr::Local(ip) => ip,
            TaggedIpAddr::Remote(ip) => ip,
        }
    }
}

impl DerefMut for TaggedIpAddr {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            TaggedIpAddr::Local(ip) => ip,
            TaggedIpAddr::Remote(ip) => ip,
        }
    }
}

impl IpPair {
    pub fn new(ip1: IpAddr, ip2: IpAddr) -> Self {
        IpPair {
            pair: if ip1 < ip2 { (ip1, ip2) } else { (ip2, ip1) },
        }
    }

    pub fn from_packet(packet: &ParsedPacket) -> Self {
        IpPair::new(packet.dst_ip, packet.src_ip)
    }

    pub fn get_pair(&self) -> (IpAddr, IpAddr) {
        self.pair
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        self.pair.0 == ip || self.pair.1 == ip
    }

    /// Used for keeping track of seen IP addrs.
    /// Since IpPair is a bi-directional pair, this function will return the IP that does not match the input IP.
    /// If the IP is not in the pair, it will return the pair.
    pub fn get_non_matching(&self, ip: IpAddr) -> Vec<IpAddr> {
        if self.pair.0 == ip {
            vec![self.pair.1]
        } else if self.pair.1 == ip {
            vec![self.pair.0]
        } else {
            vec![self.pair.0, self.pair.1]
        }
    }
}

impl Display for IpPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.pair.0, self.pair.1)
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub struct StreamKey {
    pub ports: (Option<u16>, Option<u16>),
    pub protocol: IpNextHeaderProtocol,
}

impl StreamKey {
    pub fn new(protocol: IpNextHeaderProtocol, port1: Option<u16>, port2: Option<u16>) -> Self {
        StreamKey {
            ports: if port1 < port2 {
                (port1, port2)
            } else {
                (port2, port1)
            },
            protocol,
        }
    }

    pub fn from_packet(packet: &ParsedPacket) -> Self {
        match &packet.transport {
            TransportPacket::TCP {
                src_port, dst_port, ..
            }
            | TransportPacket::UDP {
                src_port, dst_port, ..
            } => StreamKey::new(
                packet.transport.get_ip_proto(),
                Some(*src_port),
                Some(*dst_port),
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

pub fn from_iperf_connected(
    connected: &Connected,
    proto: IpNextHeaderProtocol,
) -> (StreamKey, IpPair) {
    (
        StreamKey::new(
            proto,
            Some(connected.local_port as u16),
            Some(connected.remote_port as u16),
        ),
        IpPair::new(
            connected.local_host.parse().unwrap(),
            connected.remote_host.parse().unwrap(),
        ),
    )
}

macro_rules! from_net_entry {
    ($func_name:ident, $net_type:ty) => {
        pub fn $func_name(
            entry: &$net_type,
            protocol: IpNextHeaderProtocol,
        ) -> (StreamKey, IpPair) {
            (
                StreamKey::new(
                    protocol,
                    Some(entry.local_address.port()),
                    Some(entry.remote_address.port()),
                ),
                IpPair::new(entry.local_address.ip(), entry.remote_address.ip()),
            )
        }
    };
}

from_net_entry!(from_tcp_net_entry, TcpNetEntry);
from_net_entry!(from_udp_net_entry, UdpNetEntry);
