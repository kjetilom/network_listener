use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, SystemTime};

use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use procfs::net::{TcpState, UdpState};

use super::super::packet::{
    direction::Direction,
    packet_builder::ParsedPacket,
    transport_packet::{TcpFlags, TransportPacket},
};

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

    pub fn default(protocol: IpNextHeaderProtocol) -> Self {
        panic!("Not implemented");
    }
}

pub trait DefaultState {
    fn default(protocol: IpNextHeaderProtocol) -> Self;
    fn register_packet(&mut self, packet: &ParsedPacket);
}

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

/// Single struct to represent a sent or received packet with optional RTT.
#[derive(Debug)]
struct SentPacket {
    len: u32,
    sent_time: SystemTime,
    retransmissions: u32,
    rtt: Option<RTT>,
}

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

/// Wrap-around aware comparison
fn seq_cmp(a: u32, b: u32) -> i32 {
    a.wrapping_sub(b) as i32
}

fn seq_less_equal(a: u32, b: u32) -> bool {
    seq_cmp(a, b) <= 0
}

#[derive(Debug, Clone)]
pub struct RTT {
    pub rtt: Duration,
    pub packet_size: u32,
    pub timestamp: SystemTime,
}

#[derive(Debug)]
pub struct TcpStats {
    pub total_retransmissions: u32,
    pub total_unique_packets: u32,
    pub recv: VecDeque<SentPacket>,
    pub sent: VecDeque<SentPacket>,
    pub state: Option<TcpState>,
    pub initial_rtt: Option<RTT>,
}

impl TcpStats {
    pub fn new() -> Self {
        TcpStats {
            total_retransmissions: 0,
            total_unique_packets: 0,
            recv: VecDeque::with_capacity(1000),
            sent: VecDeque::with_capacity(1000),
            state: None,
            initial_rtt: None,
        }
    }

    pub fn register_data_received(&mut self, p: SentPacket) {
        self.recv.push_front(p);
    }

    pub fn register_data_sent(&mut self, p: SentPacket) {
        self.sent.push_front(p);
    }

    pub fn estimate_bandwidth(&self) -> Option<f64> {
        // Not implemented in this snippet
        Some(0.0)
    }
}

#[derive(Debug)]
pub struct TcpTracker {
    sent_packets: BTreeMap<u32, SentPacket>,
    initial_sequence_local: Option<u32>,
    pub stats: TcpStats,
    total_bytes_sent: u64,
    total_bytes_acked: u64,
}

impl TcpTracker {
    pub fn new() -> Self {
        TcpTracker {
            sent_packets: BTreeMap::new(),
            initial_sequence_local: None,
            stats: TcpStats::new(),
            total_bytes_sent: 0,
            total_bytes_acked: 0,
        }
    }

    pub fn register_packet(&mut self, packet: &ParsedPacket) {
        self.handle_packet(packet);
    }

    /// Handles both incoming and outgoing logic here.
    fn handle_packet(&mut self, packet: &ParsedPacket) {
        if let TransportPacket::TCP {
            sequence,
            acknowledgment,
            payload_len,
            flags,
            ..
        } = &packet.transport
        {
            match packet.direction {
                Direction::Incoming => {
                    // Record data if it’s not just a pure ACK.
                    if !flags.is_ack() || *payload_len != 0 {
                        self.stats.register_data_received(SentPacket {
                            len: *payload_len as u32,
                            sent_time: packet.timestamp,
                            retransmissions: 0,
                            rtt: None,
                        });
                    }

                    // Update acked packets if possible.
                    if let Some(initial_seq_local) = self.initial_sequence_local {
                        let ack = acknowledgment.wrapping_sub(initial_seq_local);
                        self.total_bytes_acked = ack as u64;
                        self.update_acked_packets(*acknowledgment, packet.timestamp);
                    }
                }
                Direction::Outgoing => {
                    if flags.is_ack() && *payload_len == 0 {
                        // Pure ACK from us, ignore.
                        return;
                    }

                    if flags.is_syn() && !flags.is_ack() {
                        self.initial_sequence_local = Some(*sequence);
                    }

                    // Simple TCP state machine transitions.
                    if flags.is_fin() || flags.is_rst() {
                        self.initial_sequence_local = None;
                        self.stats.state = Some(TcpState::Close);
                    } else {
                        self.stats.state = Some(TcpState::Established);
                    }

                    self.total_bytes_sent += packet.total_length as u64;
                    self.track_outgoing_packet(*sequence, *payload_len, packet.timestamp, flags);
                }
            }
        }
    }

    fn track_outgoing_packet(
        &mut self,
        sequence: u32,
        payload_len: u16,
        timestamp: SystemTime,
        flags: &TcpFlags,
    ) {
        if self.initial_sequence_local.is_none() {
            // If we have no initial sequence, treat this as the start
            self.initial_sequence_local = Some(sequence);
        }

        if let Some(_initial_seq) = self.initial_sequence_local {
            let mut len = payload_len as u32;
            if flags.is_syn() || flags.is_fin() {
                len += 1;
            }

            if len > 0 {
                match self.sent_packets.get_mut(&sequence) {
                    Some(existing) => {
                        existing.retransmissions += 1;
                        self.stats.total_retransmissions += 1;
                    }
                    None => {
                        let new_packet = SentPacket {
                            len,
                            sent_time: timestamp,
                            retransmissions: 0,
                            rtt: None,
                        };
                        self.stats.total_unique_packets += 1;
                        self.sent_packets.insert(sequence, new_packet);
                    }
                }
            }
        }
    }

    fn update_acked_packets(&mut self, ack: u32, ack_timestamp: SystemTime) {
        let mut keys_to_remove = Vec::new();

        for (&seq, sent_packet) in self.sent_packets.iter_mut() {
            if seq_less_equal(seq + sent_packet.len, ack) {
                // If packet was never retransmitted, measure RTT
                if sent_packet.retransmissions == 0 {
                    if let Ok(rtt_duration) = ack_timestamp.duration_since(sent_packet.sent_time) {
                        sent_packet.rtt = Some(RTT {
                            rtt: rtt_duration,
                            packet_size: sent_packet.len,
                            timestamp: ack_timestamp,
                        });
                        // Optionally store first RTT for future usage
                        if self.stats.initial_rtt.is_none() {
                            self.stats.initial_rtt = sent_packet.rtt.clone();
                        }
                    }
                }
                keys_to_remove.push(seq);
            } else {
                // Since sent_packets is sorted, we can break early
                break;
            }
        }

        for seq in keys_to_remove {
            if let Some(p) = self.sent_packets.remove(&seq) {
                self.stats.register_data_sent(p);
            }
        }
    }
}

impl DefaultState for TcpTracker {
    fn default(_protocol: IpNextHeaderProtocol) -> Self {
        Self::new()
    }

    fn register_packet(&mut self, packet: &ParsedPacket) {
        self.register_packet(packet);
    }
}
