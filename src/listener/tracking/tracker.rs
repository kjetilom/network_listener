use std::time::SystemTime;

use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};

use crate::{
    tcp_tracker::TcpTracker, udp_tracker::UdpTracker, Direction, GenericTracker, ParsedPacket,
};

use super::tcp_tracker::Burst;

pub trait DefaultState {
    fn default(protocol: IpNextHeaderProtocol) -> Self;
    fn register_packet(&mut self, packet: &ParsedPacket) -> Option<(Burst, Direction)>;
}

#[derive(Debug)]
pub enum TrackerState {
    Tcp(TcpTracker),
    Udp(UdpTracker),
    Other(GenericTracker),
}

impl DefaultState for TrackerState {
    fn register_packet(&mut self, packet: &ParsedPacket) -> Option<(Burst, Direction)> {
        match self {
            TrackerState::Tcp(tracker) => tracker.register_packet(packet),
            TrackerState::Udp(tracker) => tracker.register_packet(packet),
            TrackerState::Other(tracker) => tracker.register_packet(packet),
        }
    }

    fn default(protocol: IpNextHeaderProtocol) -> Self {
        match protocol {
            IpNextHeaderProtocols::Tcp => TrackerState::Tcp(TcpTracker::new()),
            IpNextHeaderProtocols::Udp => TrackerState::Udp(UdpTracker::default()),
            _ => TrackerState::Other(GenericTracker::new(protocol)),
        }
    }
}

#[derive(Debug)]
pub struct Tracker<TState> {
    pub last_registered: SystemTime,
    pub protocol: IpNextHeaderProtocol,
    pub state: TState,
}

impl<TState: DefaultState> Tracker<TState> {
    pub fn new(timestamp: SystemTime, protocol: IpNextHeaderProtocol) -> Self {
        Self {
            last_registered: timestamp,
            protocol,
            state: TState::default(protocol),
        }
    }

    pub fn register_packet(&mut self, packet: &ParsedPacket) -> Option<(Burst, Direction)> {
        self.last_registered = packet.timestamp;
        self.state.register_packet(packet)
    }
}
