use std::net::IpAddr;
use std::hash::{Hash, Hasher};

use pcap::Address;

use super::traffic_analyzer::{Direction, ParsedPacket, Protocol};

#[derive(Debug, Clone)]
pub struct Socket {
    pub ip: IpAddr,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct Connection {
    pub local: Socket,
    pub remote: Socket,
    pub protocol: Protocol,
}

impl Connection {
    pub fn new(local: Socket, remote: Socket, protocol: Protocol) -> Connection {
        Connection {
            local,
            remote,
            protocol,
        }
    }

    pub fn from(packet: &ParsedPacket) -> Option<Connection> {
        if packet.protocol.protocol() == Protocol::Other {
            return None;
        }

        let dir = &packet.direction;

        let src_ip = packet.src_ip;
        let dst_ip = packet.dst_ip;

        let src_port = packet.src_port();
        let dst_port = packet.dst_port();

        let sock1 = Socket {
            ip: src_ip,
            port: src_port,
        };
        let sock2 = Socket {
            ip: dst_ip,
            port: dst_port,
        };

        match dir {
            Direction::Incoming => Some(Connection::new(sock2, sock1, packet.protocol.protocol())),
            Direction::Outgoing => Some(Connection::new(sock1, sock2, packet.protocol.protocol())),
        }


    }
}

impl PartialEq for Connection {
    fn eq(&self, other: &Self) -> bool {
        self.local.ip == other.local.ip
            && self.local.port == other.local.port
            && self.remote.ip == other.remote.ip
            && self.remote.port == other.remote.port
            && self.protocol == other.protocol
    }
}

impl Eq for Connection {}

impl Hash for Connection {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.local.ip.hash(state);
        self.local.port.hash(state);
        self.remote.ip.hash(state);
        self.remote.port.hash(state);
        self.protocol.hash(state);
    }
}