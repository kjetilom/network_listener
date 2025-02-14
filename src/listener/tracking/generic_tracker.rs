use crate::listener::packet::ParsedPacket;
use crate::listener::tracking::tracker::DefaultState;
use crate::DataPacket;
use pnet::packet::ip::IpNextHeaderProtocol;

#[derive(Debug)]
pub struct GenericTracker;

impl Default for GenericTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl GenericTracker {
    pub fn new() -> Self {
        GenericTracker
    }

    pub fn register_packet(&mut self, packet: &ParsedPacket) -> Vec<DataPacket> {
        vec![DataPacket::from_packet(packet)]
    }
}

impl DefaultState for GenericTracker {
    fn default(_protocol: IpNextHeaderProtocol) -> Self {
        Self::new()
    }
    fn register_packet(&mut self, packet: &ParsedPacket) -> Vec<DataPacket> {
        self.register_packet(packet)
    }
}
