use crate::listener::packet::ParsedPacket;
use crate::{Direction, PacketType};
use pnet::packet::ip::IpNextHeaderProtocol;

use super::tcp_tracker::Burst;

#[derive(Debug)]
pub struct GenericTracker {
    pub protocol: IpNextHeaderProtocol,
    burst_in: Vec<PacketType>,
    burst_out: Vec<PacketType>,
    last_in: std::time::SystemTime,
    last_out: std::time::SystemTime,
}



impl GenericTracker {

    pub fn new(protocol: IpNextHeaderProtocol) -> Self {
        GenericTracker {
            protocol,
            burst_in: Vec::new(),
            burst_out: Vec::new(),
            last_in: std::time::SystemTime::UNIX_EPOCH,
            last_out: std::time::SystemTime::UNIX_EPOCH,
        }
    }

    pub fn register_packet(&mut self, packet: &ParsedPacket) -> Option<(Burst, Direction)> {
        let mut ret = Vec::new();

        let (mut burst, last) = match packet.direction {
            Direction::Incoming => {
                (&mut self.burst_in, &mut self.last_in)
            }
            Direction::Outgoing => {
                (&mut self.burst_out, &mut self.last_out)
            }
        };

        if let Ok(dur) = packet.timestamp.duration_since(*last) {
            if dur > std::time::Duration::from_secs(1) || burst.len() == 100 {
                std::mem::swap(&mut ret, &mut burst);
            }
        }

        burst.push(PacketType::from_packet(packet));
        *last = packet.timestamp;

        if ret.is_empty() {
            None
        } else {
            Some((Burst::Udp(ret), packet.direction))
        }
    }
}
