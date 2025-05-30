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
    pub fn register_packet(&mut self, packet: &ParsedPacket) -> Option<(Burst, Direction)> {
        let mut ret = Vec::new();

        let (burst, last) = match packet.direction {
            Direction::Incoming => (&mut self.burst_in, &mut self.last_in),
            Direction::Outgoing => (&mut self.burst_out, &mut self.last_out),
        };

        if let Ok(dur) = packet.timestamp.duration_since(*last) {
            if dur > std::time::Duration::from_secs(1) || burst.len() == 100 {
                std::mem::swap(&mut ret, burst);
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

    pub fn take_bursts(&mut self) -> (Burst, Burst) {
        let mut in_burst = Vec::new();
        let mut out_burst = Vec::new();
        std::mem::swap(&mut in_burst, &mut self.burst_in);
        std::mem::swap(&mut out_burst, &mut self.burst_out);
        (Burst::Udp(in_burst), Burst::Udp(out_burst))
    }
}
