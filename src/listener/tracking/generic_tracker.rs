use crate::listener::packet::ParsedPacket;
use crate::PacketType;
use pnet::packet::ip::IpNextHeaderProtocol;

#[derive(Debug)]
pub struct GenericTracker {
    pub protocol: IpNextHeaderProtocol,
}

impl GenericTracker {
    pub fn new(protocol: IpNextHeaderProtocol) -> Self {
        GenericTracker {protocol}
    }
    pub fn register_packet(&mut self, packet: &ParsedPacket) -> Vec<PacketType> {
        vec![PacketType::from_packet(packet)]
    }
}
