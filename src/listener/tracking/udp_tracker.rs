use std::time::SystemTime;

use procfs::net::UdpState;

use crate::Direction;
use crate::PacketType;
use crate::ParsedPacket;

use super::tcp_tracker::Burst;

#[derive(Debug)]
pub struct UdpTracker {
    pub state: Option<UdpState>,
    burst_in: Vec<PacketType>,
    burst_out: Vec<PacketType>,
    last_in: SystemTime,
    last_out: SystemTime,
}

impl Default for UdpTracker {
    fn default() -> Self {
        UdpTracker {
            state: Some(UdpState::Established),
            burst_in: Vec::new(),
            burst_out: Vec::new(),
            last_in: SystemTime::UNIX_EPOCH,
            last_out: SystemTime::UNIX_EPOCH,
        }
    }
}

impl UdpTracker {

    pub fn register_packet(&mut self, packet: &ParsedPacket) -> (Burst, Direction) {
        let mut ret = Vec::new();

        let (mut burst, mut last) = match packet.direction {
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

        (Burst::Udp(ret), packet.direction)
    }
}
