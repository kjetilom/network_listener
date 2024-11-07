use std::fmt::{self, Display};
use std::net::IpAddr;
use std::hash::{Hash, Hasher};

use procfs::net::{TcpNetEntry, UdpNetEntry};

use super::parser::{ParsedPacket, TransportPacket};

#[derive(Debug, Clone)]
pub struct StreamId {
    local_ip: IpAddr,
    local_port: u16,
    remote_ip: IpAddr,
    remote_port: u16,
}

/*
 * Implementing PartialEq, Eq, and Hash allows
 *  us to use StreamId as a key in a HashMap.
 */
impl PartialEq for StreamId {
    fn eq(&self, other: &Self) -> bool {
        self.local_ip == other.local_ip
            && self.local_port == other.local_port
            && self.remote_ip == other.remote_ip
            && self.remote_port == other.remote_port
    }
}

impl Eq for StreamId {}

impl Hash for StreamId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.local_ip.hash(state);
        self.local_port.hash(state);
        self.remote_ip.hash(state);
        self.remote_port.hash(state);
    }
}

impl From<&TcpNetEntry> for StreamId {
    fn from(entry: &TcpNetEntry) -> Self {
        StreamId {
            local_ip: entry.local_address.ip(),
            local_port: entry.local_address.port(),
            remote_ip: entry.remote_address.ip(),
            remote_port: entry.remote_address.port(),
        }
    }
}

impl From<&UdpNetEntry> for StreamId {
    fn from(entry: &UdpNetEntry) -> Self {
        StreamId {
            local_ip: entry.local_address.ip(),
            local_port: entry.local_address.port(),
            remote_ip: entry.remote_address.ip(),
            remote_port: entry.remote_address.port(),
        }
    }
}

impl StreamId {
    pub fn new(local_ip: IpAddr, local_port: u16, remote_ip: IpAddr, remote_port: u16) -> Self {
        StreamId {
            local_ip,
            local_port,
            remote_ip,
            remote_port,
        }
    }

    pub fn from_pcap(packet: &ParsedPacket, own_ip: IpAddr) -> Self {
        if let TransportPacket::TCP {src_port, dst_port, ..}
            | TransportPacket::UDP {src_port, dst_port, ..}
            = &packet.transport
        {
            if packet.src_ip == own_ip {
                StreamId {
                    local_ip: packet.src_ip,
                    local_port: *src_port,
                    remote_ip: packet.dst_ip,
                    remote_port: *dst_port,
                }
            } else {
                StreamId {
                    local_ip: packet.dst_ip,
                    local_port: *dst_port,
                    remote_ip: packet.src_ip,
                    remote_port: *src_port,
                }
            }
        } else {
            panic!("Packet is not a TCP packet");
        }
    }
}

impl Display for StreamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{} -> {}:{}", self.local_ip, self.local_port, self.remote_ip, self.remote_port
        )
    }
}
