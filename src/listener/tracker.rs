use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use super::packet::packet_builder::ParsedPacket;
use super::packet::direction::Direction;
use super::packet::transport_packet::{TransportPacket, TcpFlags};
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
    fn register_packet(&mut self, packet: &ParsedPacket) -> bool{
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

    pub fn register_packet(&mut self, packet: &ParsedPacket) -> bool {
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
        self.state.register_packet(packet)
    }
}

#[derive(Debug)]
pub struct GenericTracker {}
impl GenericTracker {
    pub fn new() -> Self {
        GenericTracker {}
    }

    pub fn register_packet(&mut self, _packet: &ParsedPacket) -> bool {true}
}

/// Represents a sent TCP packet with its sequence number, length, send time, and retransmission count.
#[derive(Debug)]
struct SentPacket {
    len: u32,
    sent_time: SystemTime,
    retransmissions: u32,
    total_packet_size: u32,
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
}

impl UdpTracker {
    pub fn new() -> Self {
        UdpTracker { state: Some(UdpState::Established) }
    }

    pub fn register_packet(&mut self, _packet: &ParsedPacket) -> bool {true}
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
    pub rtts: VecDeque<RTT>,
    pub state: Option<TcpState>,
    pub initial_rtt: Option<RTT>,
}

impl TcpStats {
    pub fn new() -> Self {
        TcpStats {
            total_retransmissions: 0,
            total_unique_packets: 0,
            rtts: VecDeque::with_capacity(1000),
            state: None,
            initial_rtt: None,
        }
    }

    pub fn register_rtt(&mut self, rtt: RTT, flags: &TcpFlags) {
        if flags.is_syn() {
            self.initial_rtt = Some(rtt.clone());
            self.rtts.clear();
            self.rtts.push_front(rtt);
        } else if flags.is_fin() {
            self.initial_rtt = None;
            self.rtts.clear();
        } else {
            self.rtts.push_front(rtt);
        }
    }

    pub fn min_rtt(&self) -> Option<Duration> {
        self.rtts.iter().map(|rtt| rtt.rtt).min()
    }

    pub fn estimate_bandwidth(&self) -> Option<f64> {
        if self.rtts.is_empty() {
            return None;
        }

        let mut min_rtt = Duration::MAX;
        let mut max_throughput: f64 = 0.0;
        let mut avg_rtt = 0.0;

        for rtt in &self.rtts {
            min_rtt = min_rtt.min(rtt.rtt);
            let throughput = (rtt.packet_size as f64) / rtt.rtt.as_secs_f64();
            max_throughput = max_throughput.max(throughput);
            avg_rtt += rtt.rtt.as_secs_f64();
        }

        // Get the most recent RTT for estimation or use a moving average
        avg_rtt /= self.rtts.len() as f64;

        // Estimate bandwidth using the formula
        let bandwidth = max_throughput * (avg_rtt / min_rtt.as_secs_f64());
        println!("{:?} {:?} {:?} {:?}", min_rtt, max_throughput, avg_rtt, bandwidth);

        Some(bandwidth)
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

    fn register_packet(&mut self, packet: &ParsedPacket) -> bool {
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
    ) -> bool {
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

            if flags.is_ack() && *payload_len == 0 {
                return false;
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
                            total_packet_size: packet.total_length,
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
        true
    }

    pub fn handle_incoming_packet(&mut self, packet: &ParsedPacket) -> bool {
        if let TransportPacket::TCP {
            acknowledgment,
            flags,
            payload_len,
            ..
        } = &packet.transport
        {
            // Ignore non ack packets if
            if !flags.is_ack() || *payload_len != 0 {
                return false;
            }

            if let Some(initial_seq_local) = self.initial_sequence_local {
                let ack = acknowledgment.wrapping_sub(initial_seq_local);

                let bytes_acked = ack as u64;
                self.total_bytes_acked = bytes_acked;

                let mut keys_to_remove = Vec::new();

                for (&seq, sent_packet) in &self.sent_packets {
                    //dbg!(seq, acknowledgment, sent_packet);
                    if seq_less_equal(seq + sent_packet.len, *acknowledgment) {
                        if sent_packet.retransmissions == 0 {
                            if let Ok(rtt) = packet.timestamp.duration_since(sent_packet.sent_time) {
                                self.stats.register_rtt(RTT {
                                    rtt,
                                    packet_size: acknowledgment-seq,
                                    timestamp: packet.timestamp,
                                },
                                flags
                                );
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
                    self.sent_packets.remove(&seq);
                }
            }
        }
        true
    }
}
