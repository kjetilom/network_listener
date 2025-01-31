use std::fmt::{self, Display};
use std::hash::Hash;
use std::net::IpAddr;

use pnet::packet::ip::IpNextHeaderProtocol;
use procfs::net::{TcpNetEntry, UdpNetEntry};

use super::super::packet::ParsedPacket;
use super::super::packet::TransportPacket;

#[derive(Debug, Hash, Eq, PartialEq)]
pub struct IpPair {
    pair: (IpAddr, IpAddr),
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

    /// Used for keeping track of seen IP addrs.
    /// Since IpPair is a bi-directional pair, this function will return the IP that does not match the input IP.
    /// If the IP is not in the pair, it will return the pair.
    pub fn get_non_matching(&self, ip: IpAddr) -> (IpAddr, Option<IpAddr>) {
        if self.pair.0 == ip {
            (self.pair.1, None)
        } else if self.pair.1 == ip {
            (self.pair.0, None)
        } else {
            // If the IP is not in the pair, return the pair
            (self.pair.0, Some(self.pair.1))
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