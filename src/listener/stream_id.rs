use std::fmt::{self, Display};
use std::net::IpAddr;
use std::hash::{Hash, Hasher};

use procfs::net::TcpNetEntry;

use super::parser::ParsedPacket;

#[derive(Debug, Clone)]
pub struct TcpStreamId {
    local_ip: IpAddr,
    local_port: u16,
    remote_ip: IpAddr,
    remote_port: u16,
}

/*
 * Implementing PartialEq, Eq, and Hash allows
 *  us to use TcpStreamId as a key in a HashMap.
 */
impl PartialEq for TcpStreamId {
    fn eq(&self, other: &Self) -> bool {
        self.local_ip == other.local_ip
            && self.local_port == other.local_port
            && self.remote_ip == other.remote_ip
            && self.remote_port == other.remote_port
    }
}

impl Eq for TcpStreamId {}

impl Hash for TcpStreamId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.local_ip.hash(state);
        self.local_port.hash(state);
        self.remote_ip.hash(state);
        self.remote_port.hash(state);
    }
}

impl From<&TcpNetEntry> for TcpStreamId {
    fn from(entry: &TcpNetEntry) -> Self {
        TcpStreamId {
            local_ip: entry.local_address.ip(),
            local_port: entry.local_address.port(),
            remote_ip: entry.remote_address.ip(),
            remote_port: entry.remote_address.port(),
        }
    }
}

impl TcpStreamId {
    pub fn new(local_ip: IpAddr, local_port: u16, remote_ip: IpAddr, remote_port: u16) -> Self {
        TcpStreamId {
            local_ip,
            local_port,
            remote_ip,
            remote_port,
        }
    }

    pub fn from_pcap(packet: &ParsedPacket, own_ip: IpAddr) -> Self {
        if packet.src_ip == own_ip {
            TcpStreamId {
                local_ip: packet.src_ip,
                local_port: packet.src_port,
                remote_ip: packet.dst_ip,
                remote_port: packet.dst_port,
            }
        } else {
            TcpStreamId {
                local_ip: packet.dst_ip,
                local_port: packet.dst_port,
                remote_ip: packet.src_ip,
                remote_port: packet.src_port,
            }
        }
    }
}

impl Display for TcpStreamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{} -> {}:{}", self.local_ip, self.local_port, self.remote_ip, self.remote_port
        )
    }
}