use std::time::SystemTime;

use pnet::packet::ip::IpNextHeaderProtocol;
use procfs::net::UdpState;

use super::super::packet::{
    direction::Direction,
    transport_packet::TransportPacket,
};
use crate::listener::packet::packet_builder::ParsedPacket;

use super::tracker::{DefaultState, SentPacket};


#[derive(Debug)]
pub struct UdpTracker {
    pub state: Option<UdpState>,
    outgoing_packets: Vec<SentPacket>,
    incoming_packets: Vec<SentPacket>,
}

impl UdpTracker {
    pub fn new() -> Self {
        UdpTracker {
            state: Some(UdpState::Established),
            outgoing_packets: Vec::new(),
            incoming_packets: Vec::new(),
        }
    }

    fn remove_outdated_packets(&mut self) {
        let now = SystemTime::now();
        self.incoming_packets.retain(|p| {
            now.duration_since(p.sent_time)
               .map(|dur| dur.as_secs() <= 10)
               .unwrap_or(false)
        });
    }
}

impl DefaultState for UdpTracker {
    fn default(_protocol: IpNextHeaderProtocol) -> Self {
        Self::new()
    }

    fn register_packet(&mut self, packet: &ParsedPacket) {
        if let TransportPacket::UDP { .. } = packet.transport {
            let storage = match packet.direction {
                Direction::Incoming => &mut self.incoming_packets,
                Direction::Outgoing => &mut self.outgoing_packets,
            };
            storage.push(SentPacket {
                len: packet.total_length as u32,
                sent_time: packet.timestamp,
                retransmissions: 0,
                rtt: None,
            });
            self.remove_outdated_packets();
        }
    }
}