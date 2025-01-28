use std::time::{Duration, SystemTime};

use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};

use super::{
    super::packet::packet_builder::ParsedPacket,
    generic_tracker::GenericTracker,
    tcp_tracker::TcpTracker,
    udp_tracker::UdpTracker,
};

/// Single struct to represent a sent or received packet with optional RTT.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct SentPacket {
    pub len: u32,
    pub sent_time: SystemTime,
    pub retransmissions: u32,
    pub rtt: Option<Duration>, // RTT to ack the segment
}

pub trait DefaultState {
    fn default(protocol: IpNextHeaderProtocol) -> Self;
    fn register_packet(&mut self, packet: &ParsedPacket);
}

#[derive(Debug)]
pub enum TrackerState {
    Tcp(TcpTracker),
    Udp(UdpTracker),
    Other(GenericTracker),
}

impl DefaultState for TrackerState {
    fn register_packet(&mut self, packet: &ParsedPacket) {
        match self {
            TrackerState::Tcp(tracker) => tracker.register_packet(packet),
            TrackerState::Udp(tracker) => tracker.register_packet(packet),
            TrackerState::Other(tracker) => tracker.register_packet(packet),
        }
    }

    fn default(protocol: IpNextHeaderProtocol) -> Self {
        match protocol {
            IpNextHeaderProtocols::Tcp => TrackerState::Tcp(TcpTracker::new()),
            IpNextHeaderProtocols::Udp => TrackerState::Udp(UdpTracker::new()),
            _ => TrackerState::Other(GenericTracker::new()),
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

    pub fn register_packet(&mut self, packet: &ParsedPacket) {
        self.state.register_packet(packet);
    }

    pub fn default(_protocol: IpNextHeaderProtocol) -> Self {
        panic!("Not implemented");
    }
}
