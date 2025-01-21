use std::fmt::{self, Display};
use std::net::IpAddr;
use std::cmp::Ordering;
use std::hash::Hash;

use procfs::net::{TcpNetEntry, UdpNetEntry};
use pnet::packet::ip::IpNextHeaderProtocol;

use super::packet::packet_builder::ParsedPacket;
use super::packet::transport_packet::TransportPacket;

/// Represents a key for identifying connections
#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub enum ConnectionKey {
    StreamId {
        protocol: IpNextHeaderProtocol,
        local_ip: IpAddr,
        local_port: u16,
        remote_ip: IpAddr,
        remote_port: u16,
    },
    IpPair {
        protocol: IpNextHeaderProtocol,
        local_ip: IpAddr,
        remote_ip: IpAddr,
    },
}

/// Sort (ip1, port1) and (ip2, port2) so the smaller is considered "local".
/// If ports are None, we fall back to an IpPair.
fn symmetrical_key(
    protocol: IpNextHeaderProtocol,
    ip1: IpAddr,
    port1: Option<u16>,
    ip2: IpAddr,
    port2: Option<u16>,
) -> ConnectionKey {
    // Compare two (IpAddr, Option<u16>) tuples.
    // For convenience, define a small comparator inline:
    let cmp = match ip1.cmp(&ip2) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => port1.cmp(&port2),
    };

    match (port1, port2) {
        (Some(lport), Some(rport)) => {
            // Both sides have ports => StreamId
            match cmp {
                Ordering::Less | Ordering::Equal => ConnectionKey::StreamId {
                    protocol,
                    local_ip: ip1,
                    local_port: lport,
                    remote_ip: ip2,
                    remote_port: rport,
                },
                Ordering::Greater => ConnectionKey::StreamId {
                    protocol,
                    local_ip: ip2,
                    local_port: rport,
                    remote_ip: ip1,
                    remote_port: lport,
                },
            }
        }
        _ => {
            // At least one side has no port => IpPair
            if cmp == Ordering::Greater {
                ConnectionKey::IpPair {
                    protocol,
                    local_ip: ip2,
                    remote_ip: ip1,
                }
            } else {
                ConnectionKey::IpPair {
                    protocol,
                    local_ip: ip1,
                    remote_ip: ip2,
                }
            }
        }
    }
}

macro_rules! from_net_entry {
    ($func_name:ident, $net_type:ty) => {
        pub fn $func_name(entry: &$net_type, protocol: IpNextHeaderProtocol) -> Self {
            symmetrical_key(
                protocol,
                entry.local_address.ip(),
                Some(entry.local_address.port()),
                entry.remote_address.ip(),
                Some(entry.remote_address.port()),
            )
        }
    };
}

impl ConnectionKey {
    from_net_entry!(from_tcp_net_entry, TcpNetEntry);
    from_net_entry!(from_udp_net_entry, UdpNetEntry);

    /// Create a symmetrical ConnectionKey from a ParsedPacket
    pub fn from_pcap(packet: &ParsedPacket) -> Self {
        match &packet.transport {
            TransportPacket::TCP { src_port, dst_port, .. }
            | TransportPacket::UDP { src_port, dst_port, .. } => {
                symmetrical_key(
                    packet.transport.get_ip_proto(),
                    packet.src_ip,
                    Some(*src_port),
                    packet.dst_ip,
                    Some(*dst_port),
                )
            }
            _ => {
                symmetrical_key(
                    packet.transport.get_ip_proto(),
                    packet.src_ip,
                    None,
                    packet.dst_ip,
                    None,
                )
            }
        }
    }

    pub fn get_remote_ip(&self) -> IpAddr {
        match self {
            ConnectionKey::StreamId { remote_ip, .. } => *remote_ip,
            ConnectionKey::IpPair { remote_ip, .. } => *remote_ip,
        }
    }

    pub fn get_protocol(&self) -> IpNextHeaderProtocol {
        match self {
            ConnectionKey::StreamId { protocol, .. } => *protocol,
            ConnectionKey::IpPair { protocol, .. } => *protocol,
        }
    }
}

impl Display for ConnectionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionKey::StreamId {
                protocol,
                local_ip,
                local_port,
                remote_ip,
                remote_port,
            } => {
                write!(
                    f,
                    "{} : {}:{} -> {}:{}",
                    protocol, local_ip, local_port, remote_ip, remote_port
                )
            }
            ConnectionKey::IpPair {
                protocol,
                local_ip,
                remote_ip,
            } => write!(f, "{} : {} -> {}", protocol, local_ip, remote_ip),
        }
    }
}
