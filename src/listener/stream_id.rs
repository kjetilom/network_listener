use std::net::Ipv4Addr;
use std::hash::{Hash, Hasher};

use super::parser::ParsedPacket;

pub struct TcpStreamId {
    src_ip: Ipv4Addr,
    src_port: u16,
    dst_ip: Ipv4Addr,
    dst_port: u16,
}

impl PartialEq for TcpStreamId {
    fn eq(&self, other: &Self) -> bool {
        self.src_ip == other.src_ip
            && self.src_port == other.src_port
            && self.dst_ip == other.dst_ip
            && self.dst_port == other.dst_port
    }
}

impl Eq for TcpStreamId {}

impl Hash for TcpStreamId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.src_ip.hash(state);
        self.src_port.hash(state);
        self.dst_ip.hash(state);
        self.dst_port.hash(state);
    }
}

impl TcpStreamId {
    pub fn new(src_ip: Ipv4Addr, src_port: u16, dst_ip: Ipv4Addr, dst_port: u16) -> Self {
        TcpStreamId {
            src_ip,
            src_port,
            dst_ip,
            dst_port,
        }
    }

    pub fn from_parsed_packet(packet: &ParsedPacket) -> Self {
        TcpStreamId {
            src_ip: packet.src_ip,
            src_port: packet.src_port,
            dst_ip: packet.dst_ip,
            dst_port: packet.dst_port,
        }
    }
}