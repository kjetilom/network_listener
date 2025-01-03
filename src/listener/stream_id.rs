use std::fmt::{self, Display};
use std::net::IpAddr;
use std::hash::Hash;

use procfs::net::{TcpNetEntry, UdpNetEntry};
use pnet::packet::ip::IpNextHeaderProtocol;

use super::parser::{ParsedPacket, TransportPacket};

/// Represents a key for identifying connections, which can be either:
/// - `StreamId`: Includes local and remote IPs and ports.
/// - `IpPair`: Includes only local and remote IPs.
#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub enum ConnectionKey {
    StreamId{
        protocol: IpNextHeaderProtocol,
        local_ip: IpAddr,
        local_port: u16,
        remote_ip: IpAddr,
        remote_port: u16
    },
    IpPair{
        protocol: IpNextHeaderProtocol,
        local_ip: IpAddr,
        remote_ip: IpAddr
    }
}

// Trying out a macro to reduce code duplication
macro_rules! from_net_entry {
    ($type:ty, $name:ident) => {
        pub fn $name(entry: &$type, protocol: IpNextHeaderProtocol) -> Self {
            ConnectionKey::StreamId {
                protocol: protocol,
                local_ip: entry.local_address.ip(),
                local_port: entry.local_address.port(),
                remote_ip: entry.remote_address.ip(),
                remote_port: entry.remote_address.port(),
            }
        }
    };
}

impl ConnectionKey {
    from_net_entry!(TcpNetEntry, from_tcp_net_entry);
    from_net_entry!(UdpNetEntry, from_udp_net_entry);
}

impl ConnectionKey {
    /// Create a ConnectionKey from a ParsedPacket
    ///
    /// # Arguments
    ///
    /// * `packet` - The ParsedPacket to create the ConnectionKey from
    /// * `own_ip` - The IP address of the local machine
    ///
    /// # Returns
    ///
    /// A ConnectionKey representing the connection
    pub fn from_pcap(packet: &ParsedPacket) -> Self {
        // Determine if the packet is outgoing or incoming
        let outgoing = packet.direction.is_outgoing();

        let local_ip = if outgoing { packet.src_ip } else { packet.dst_ip };
        let remote_ip = if outgoing { packet.dst_ip } else { packet.src_ip };

        match &packet.transport {
            TransportPacket::TCP { src_port, dst_port, .. }
            | TransportPacket::UDP { src_port, dst_port, .. } => {
                ConnectionKey::StreamId {
                    protocol: packet.transport.get_ip_proto(),
                    local_ip,
                    local_port: if outgoing { *src_port } else { *dst_port },
                    remote_ip,
                    remote_port: if outgoing { *dst_port } else { *src_port },
                }
            }
            _ => {
                // For other protocols or when transport info is not available
                ConnectionKey::IpPair {
                    protocol: packet.transport.get_ip_proto(),
                    local_ip,
                    remote_ip
                }
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
            } => write!(
                f,
                "{} : {}:{} -> {}:{}", protocol, local_ip, local_port, remote_ip, remote_port
            ),
            ConnectionKey::IpPair {protocol, local_ip, remote_ip } => {
                write!(f, "{} : {} -> {}", protocol, local_ip, remote_ip)
            }
        }
    }
}
