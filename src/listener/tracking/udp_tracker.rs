use procfs::net::UdpState;

use crate::PacketType;
use crate::ParsedPacket;

#[derive(Debug)]
pub struct UdpTracker {
    pub state: Option<UdpState>,
}

impl Default for UdpTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl UdpTracker {
    pub fn new() -> Self {
        UdpTracker {
            state: Some(UdpState::Established),
        }
    }

    pub fn register_packet(&mut self, packet: &ParsedPacket) -> Vec<PacketType> {
        vec![PacketType::from_packet(packet)]
    }
}
