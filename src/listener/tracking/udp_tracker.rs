use pnet::packet::ip::IpNextHeaderProtocol;
use procfs::net::UdpState;

use crate::ParsedPacket;
use crate::{tracker::DefaultState, DataPacket};

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
}

impl DefaultState for UdpTracker {
    fn default(_protocol: IpNextHeaderProtocol) -> Self {
        Self::new()
    }

    fn register_packet(&mut self, packet: &ParsedPacket) -> Vec<DataPacket> {
        vec![DataPacket::from_packet(packet)]
    }
}
