use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use super::super::packet::packet_builder::ParsedPacket;
use super::super::packet::direction::Direction;
use super::super::packet::transport_packet::TransportPacket;
use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use procfs::net::{TcpState, UdpState};
// Use circular buffer to store RTTs
use std::collections::VecDeque;

#[derive(Debug)]
pub enum TrackerState {
    Tcp(TcpTracker),
    Udp(UdpTracker),
    Other(GenericTracker),
}

impl TrackerState {
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
pub struct Tracker<TrackerState> {
    pub last_registered: SystemTime,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub protocol: IpNextHeaderProtocol,
    pub state: TrackerState,
}

impl Tracker<TrackerState> {
    pub fn new(timestamp: SystemTime, protocol: IpNextHeaderProtocol) -> Self {
        Tracker {
            last_registered: timestamp,
            total_bytes_sent: 0,
            total_bytes_received: 0,
            protocol,
            state: TrackerState::default(protocol),
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

        // Call register_packet on state if it exists
        self.state.register_packet(packet);
    }
}

#[derive(Debug)]
pub struct GenericTracker {}
impl GenericTracker {
    pub fn new() -> Self {
        GenericTracker {}
    }

    pub fn register_packet(&mut self, _packet: &ParsedPacket) {}
}

/// Represents a sent TCP packet with its sequence number, length, send time, and retransmission count.
#[derive(Debug)]
struct SentPacket {
    len: u32,
    sent_time: SystemTime,
    retransmissions: u32,
    rtt: Option<RTT>,
}

/// Tracks TCP streams and their state.
#[derive(Debug)]
pub struct TcpTracker {
    sent_packets: BTreeMap<u32, SentPacket>,
    initial_sequence_local: Option<u32>,
    pub stats: TcpStats,
    total_bytes_sent: u64,
    total_bytes_acked: u64,
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

    fn register_packet(&mut self, packet: &ParsedPacket) {
        if let TransportPacket::UDP { .. } = &packet.transport {
            match packet.direction {
                Direction::Incoming => {
                    self.incoming_packets.push(SentPacket {
                        len: packet.total_length as u32,
                        sent_time: packet.timestamp,
                        retransmissions: 0,
                        rtt: None,
                    });
                }
                Direction::Outgoing => {
                    self.outgoing_packets.push(SentPacket {
                        len: packet.total_length as u32,
                        sent_time: packet.timestamp,
                        retransmissions: 0,
                        rtt: None,
                    });
                }
            }
            // while any packet is older than n seconds, remove it
            while let Some(packet) = self.incoming_packets.last() {
                if packet.sent_time.elapsed().unwrap().as_secs() > 10 { // Change this to a constant
                    self.incoming_packets.pop();
                } else {
                    break;
                }
            }
        }
    }
}

/// Wrap-around aware comparison
fn seq_cmp(a: u32, b: u32) -> i32 {
    (a.wrapping_sub(b)) as i32
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

    fn register_data_received(&mut self, p: SentPacket) {
        self.recv.push_front(p);
    }

    fn register_data_sent(&mut self, p: SentPacket) {
        self.sent.push_front(p);
    }

    pub fn estimate_bandwidth(&self) -> Option<f64> {
        Some(0.0)
    }
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

    fn register_packet(&mut self, packet: &ParsedPacket) {
        match packet.direction {
            Direction::Incoming => {
                self.handle_incoming_packet(packet)
            }
            Direction::Outgoing => {
                self.handle_outgoing_packet(packet)
            }
        }
    }

    pub fn handle_outgoing_packet(
        &mut self,
        packet: &ParsedPacket,
    ) {
        if let TransportPacket::TCP {
            sequence,
            payload_len,
            flags,
            ..
        } = &packet.transport
        {
            // Lets say we have a network like:
            // A <-> B <-> C
            // If the packet is marked A <-> C, it is intercepted
            //
            // This means that a stream will be registered twice for the same packet
            //
            // How do we handle this?
            //
            // If the packet is intercepted, we only care about outgoing packets where we expect an ACK
            // Ignore packets where: ACK && intercepted && data is not being sent

            // Ignore ACK packets with no payload (We are sending an ACK.)
            if flags.is_ack() && *payload_len == 0 {
                return;
            }

            if flags.is_syn() && !flags.is_ack() {
                self.initial_sequence_local = Some(*sequence);
            }

            // Simple state machine
            if flags.is_fin() || flags.is_rst() {
                self.initial_sequence_local = None;
                self.stats.state = Some(TcpState::Close);
            } else {
                self.stats.state = Some(TcpState::Established);
            }

            self.total_bytes_sent += packet.total_length as u64;

            if let Some(_initial_seq) = self.initial_sequence_local {
                let seq = *sequence;

                // Calculate the length considering SYN and FIN flags.
                let mut len = *payload_len as u32;

                if flags.is_syn() || flags.is_fin() {
                    // SYN or FIN flag
                    len += 1;
                }

                if len > 0 {
                    if let Some(sent_packet) = self.sent_packets.get_mut(&seq) {
                        // Retransmission detected.
                        sent_packet.retransmissions += 1;
                        self.stats.total_retransmissions += 1;
                        // Do not update sent_time to keep the original send time (Karn's Algorithm).
                    } else {
                        // New packet sent.
                        let sent_packet = SentPacket {
                            len,
                            sent_time: packet.timestamp,
                            retransmissions: 0,
                            rtt: None,
                        };
                        self.stats.total_unique_packets += 1;

                        self.sent_packets.insert(seq, sent_packet);
                    }
                }
            } else {
                // Since we don't know the initial sequence number,
                // we'll count the first packet as the initial one.
                self.initial_sequence_local = Some(*sequence);
            }
        }
    }

    pub fn handle_incoming_packet(&mut self, packet: &ParsedPacket) {
        if let TransportPacket::TCP {
            acknowledgment,
            flags,
            payload_len,
            ..
        } = &packet.transport
        {
            // Ignore non ack packets if
            if !flags.is_ack() || *payload_len != 0 {
                self.stats.register_data_received(SentPacket {
                    len: *payload_len as u32,
                    sent_time: packet.timestamp,
                    retransmissions: 0,
                    rtt: None,
                })
            }

            if let Some(initial_seq_local) = self.initial_sequence_local {
                let ack = acknowledgment.wrapping_sub(initial_seq_local);

                let bytes_acked = ack as u64;
                self.total_bytes_acked = bytes_acked;

                let mut keys_to_remove = Vec::new();

                for (&seq, sent_packet) in self.sent_packets.iter_mut() {
                    //dbg!(seq, acknowledgment, sent_packet);
                    if seq_less_equal(seq + sent_packet.len, *acknowledgment) {
                        if sent_packet.retransmissions == 0 {
                            if let Ok(rtt) = packet.timestamp.duration_since(sent_packet.sent_time) {
                                sent_packet.rtt = Some(RTT {
                                    rtt,
                                    packet_size: sent_packet.len,
                                    timestamp: packet.timestamp,
                                });
                            }
                        }
                        keys_to_remove.push(seq);

                    } else {
                        // Since sent packets are ordered by sequence number
                        // After reaching this point, we can break the loop
                        break;
                    }
                }

                for seq in keys_to_remove {
                    let p = self.sent_packets.remove(&seq);
                    if let Some(p) = p {
                        self.stats.register_data_sent(p);
                    }
                }
            }
        }
    }
}
