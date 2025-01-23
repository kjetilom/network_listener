use crate::listener::packet::packet_builder::ParsedPacket;
use crate::listener::tracker::tracker::DefaultState;
use pnet::packet::ip::IpNextHeaderProtocol;

#[derive(Debug)]
pub struct GenericTracker;

impl GenericTracker {
    pub fn new() -> Self {
        GenericTracker
    }

    pub fn register_packet(&mut self, _packet: &ParsedPacket) {}
}

impl DefaultState for GenericTracker {
    fn default(_protocol: IpNextHeaderProtocol) -> Self {
        Self::new()
    }
    fn register_packet(&mut self, packet: &ParsedPacket) {
        self.register_packet(packet);
    }
}