use std::time::{Duration, SystemTime};

use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};

use super::{
    super::packet::{direction::Direction, packet_builder::ParsedPacket}, generic_tracker::GenericTracker, link::DataPoint, tcp_tracker::TcpTracker, udp_tracker::UdpTracker
};

/// Single struct to represent a sent or received packet with optional RTT.
#[derive(Debug, PartialEq, Eq)]
pub struct SentPacket {
    pub len: u32,
    pub sent_time: SystemTime,
    pub retransmissions: u32,
    pub rtt: Option<Duration>, // RTT to ack the segment
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RTT {
    pub rtt: Duration,
    pub packet_size: u32,
    pub timestamp: SystemTime,
}

pub trait DefaultState {
    fn default(protocol: IpNextHeaderProtocol) -> Self;
    fn register_packet(&mut self, packet: &ParsedPacket);
    fn extract_data(&mut self) -> Vec<DataPoint>;
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

    fn extract_data(&mut self) -> Vec<DataPoint> {
        match self {
            TrackerState::Tcp(tracker) => tracker.extract_data(),
            TrackerState::Udp(tracker) => tracker.extract_data(),
            TrackerState::Other(tracker) => tracker.extract_data(),
        }
    }
}

#[derive(Debug)]
pub struct Tracker<TState> {
    pub last_registered: SystemTime,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub protocol: IpNextHeaderProtocol,
    pub state: TState,
}

impl<TState: DefaultState> Tracker<TState> {
    pub fn new(timestamp: SystemTime, protocol: IpNextHeaderProtocol) -> Self {
        Self {
            last_registered: timestamp,
            total_bytes_sent: 0,
            total_bytes_received: 0,
            protocol,
            state: TState::default(protocol),
        }
    }

    pub fn register_packet(&mut self, packet: &ParsedPacket) {
        match packet.direction {
            Direction::Incoming => {
                self.total_bytes_received += packet.total_length as u64;
            }
            Direction::Outgoing => {
                self.total_bytes_sent += packet.total_length as u64;
            }
        }
        self.last_registered = packet.timestamp;
        self.state.register_packet(packet);
    }

    pub fn default(_protocol: IpNextHeaderProtocol) -> Self {
        panic!("Not implemented");
    }
}
