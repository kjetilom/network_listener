use std::collections::{BTreeMap, HashSet, VecDeque};
use std::time::{Duration, SystemTime};

use pnet::packet::ip::IpNextHeaderProtocol;
use procfs::net::TcpState;

use crate::{
    Direction, ParsedPacket, {TcpFlags, TransportPacket},
};

use super::tracker::{DefaultState, SentPacket};

static ALPHA: f64 = 0.125; // 2/(15+1)
static THRESHOLD_FACTOR: f64 = 1.1;

/// Wrap-around aware comparison
fn seq_cmp(a: u32, b: u32) -> i32 {
    a.wrapping_sub(b) as i32
}

fn seq_less_equal(a: u32, b: u32) -> bool {
    seq_cmp(a, b) <= 0
}

#[derive(Debug)]
pub struct TcpStats {
    pub retrans_in: u32,
    pub retrans_out: u32,
    pub recv: VecDeque<SentPacket>,
    pub sent: VecDeque<SentPacket>,
    received_seqs: HashSet<u32>,
    pub state: Option<TcpState>,
    pub smoothed_rtt: Option<f64>,
    prev_smoothed_rtt: Option<f64>,
}

impl Default for TcpStats {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpStats {
    pub fn new() -> Self {
        TcpStats {
            retrans_in: 0,
            retrans_out: 0,
            recv: VecDeque::with_capacity(1000),
            sent: VecDeque::with_capacity(1000),
            received_seqs: HashSet::new(),
            state: None,
            smoothed_rtt: None,
            prev_smoothed_rtt: None,
        }
    }

    pub fn register_data_received(&mut self, mut p: SentPacket, seq: &u32) {
        if self.received_seqs.contains(seq) {
            p.retransmissions += 1;
            self.retrans_in += 1;
        }
        self.received_seqs.insert(*seq);
        self.recv.push_back(p);
    }

    pub fn register_data_sent(&mut self, p: SentPacket) {
        if let Some(rtt) = p.rtt {
            self.update_rtt(rtt);
        }
        self.sent.push_back(p);
    }

    pub fn input_recv_gap(&self) -> Option<f64> {
        if let Some(first) = self.recv.front() {
            if let Some(last) = self.recv.back() {
                return Some(
                    last.sent_time
                        .duration_since(first.sent_time)
                        .unwrap()
                        .as_secs_f64()
                        / self.recv.len() as f64,
                );
            }
        }
        None
    }

    /// Updates the smoothed RTT with a new sample and
    /// returns `Some(true)` if there's a significant increase.
    /// Otherwise, returns `Some(false)` or `None` if the measurement is invalid.
    pub fn update_rtt(&mut self, new_sample: Duration) -> Option<bool> {
        let new_rtt = new_sample.as_secs_f64();

        // Initialize the EWMA on the first valid sample
        if self.smoothed_rtt.is_none() {
            self.smoothed_rtt = Some(new_rtt);
            return Some(false); // first sample, no basis for 'increase' yet
        }

        // Calculate updated EWMA
        let old_rtt = self.smoothed_rtt.unwrap();
        let updated_rtt = old_rtt + ALPHA * (new_rtt - old_rtt);
        self.prev_smoothed_rtt = self.smoothed_rtt;
        self.smoothed_rtt = Some(updated_rtt);

        // Check if the new sample crosses a threshold above the smoothed RTT
        let threshold = THRESHOLD_FACTOR * updated_rtt;

        Some(new_rtt > threshold)
    }
}

#[derive(Debug)]
pub struct TcpTracker {
    sent_packets: BTreeMap<u32, SentPacket>,
    initial_sequence_local: Option<u32>,
    bytes_in_flight: u32,
    max_bytes_in_flight: u32,
    mss: u32,
    pub stats: TcpStats,
}

impl Default for TcpTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpTracker {
    pub fn new() -> Self {
        TcpTracker {
            sent_packets: BTreeMap::new(),
            initial_sequence_local: None,
            bytes_in_flight: 0,
            max_bytes_in_flight: 0xFFFF,
            mss: 1400,
            stats: TcpStats::new(),
        }
    }

    pub fn register_packet(&mut self, packet: &ParsedPacket) {
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
                        self.stats.register_data_received(
                            SentPacket {
                                len: *payload_len as u32,
                                sent_time: packet.timestamp,
                                retransmissions: 0,
                                rtt: None,
                            },
                            sequence,
                        );
                    }

                    // Update acked packets if possible.
                    if self.initial_sequence_local.is_some() {
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
                        self.stats.retrans_out += 1;
                    }
                    None => {
                        let new_packet = SentPacket {
                            len,
                            sent_time: timestamp,
                            retransmissions: 0,
                            rtt: None,
                        };
                        self.bytes_in_flight += len;
                        if self.bytes_in_flight > self.max_bytes_in_flight {
                            self.max_bytes_in_flight = self.bytes_in_flight;
                        } else {
                            self.max_bytes_in_flight -= self.mss / 10;
                        }
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
                self.bytes_in_flight -= sent_packet.len;
                // If packet is fully acked, calculate RTT
                if let Ok(rtt_duration) = ack_timestamp.duration_since(sent_packet.sent_time) {
                    sent_packet.rtt = Some(rtt_duration);
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
