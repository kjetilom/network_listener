use std::collections::{BTreeMap, HashSet, VecDeque};
use std::time::{Duration, SystemTime};

use pnet::packet::ip::IpNextHeaderProtocol;
use procfs::net::TcpState;

use crate::{
    tracker::DefaultState,
    DataPacket, Direction, ParsedPacket, TcpFlags, TransportPacket,
};

/// Wrap-around aware sequence comparison.
fn seq_cmp(a: u32, b: u32) -> i32 {
    a.wrapping_sub(b) as i32
}
fn seq_less_equal(a: u32, b: u32) -> bool {
    seq_cmp(a, b) <= 0
}

/// TCP statistics, including RTT estimation and received packet tracking.
#[derive(Debug)]
pub struct TcpStats {
    pub rts: u32,
    pub recv: VecDeque<DataPacket>,
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
            rts: 0,
            recv: VecDeque::with_capacity(1000),
            received_seqs: HashSet::new(),
            state: None,
            smoothed_rtt: None,
            prev_smoothed_rtt: None,
        }
    }

    /// Called when a packet is acknowledged.
    pub fn register_data_sent(&mut self, p: DataPacket) {
        if let Some(rtt) = p.rtt {
            self.update_rtt(rtt);
        }
    }

    /// Update the smoothed RTT using an EWMA.
    pub fn update_rtt(&mut self, new_sample: Duration) -> Option<bool> {
        let new_rtt = new_sample.as_secs_f64();
        if self.smoothed_rtt.is_none() {
            self.smoothed_rtt = Some(new_rtt);
            return Some(false);
        }
        let old_rtt = self.smoothed_rtt.unwrap();
        let updated_rtt = old_rtt + 0.125 * (new_rtt - old_rtt);
        self.prev_smoothed_rtt = self.smoothed_rtt;
        self.smoothed_rtt = Some(updated_rtt);
        let threshold = 1.1 * updated_rtt;
        Some(new_rtt > threshold)
    }

    pub fn register_data_received(&mut self, mut p: DataPacket, seq: u32) {
        if self.received_seqs.contains(&seq) {
            p.retransmissions += 1;
        }
        self.received_seqs.insert(seq);
        self.recv.push_back(p);
    }
}

/// TCP tracker which now tracks packets from both directions.
/// Outgoing packets (from us) are stored in `local_sent_packets` and
/// packets from the remote side are stored in `remote_sent_packets`.
#[derive(Debug)]
pub struct TcpTracker {
    local_sent_packets: BTreeMap<u32, DataPacket>,
    remote_sent_packets: BTreeMap<u32, DataPacket>,
    pub initial_sequence_local: Option<u32>,
    pub initial_sequence_remote: Option<u32>,
}

impl Default for TcpTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpTracker {
    pub fn new() -> Self {
        TcpTracker {
            local_sent_packets: BTreeMap::new(),
            remote_sent_packets: BTreeMap::new(),
            initial_sequence_local: None,
            initial_sequence_remote: None,
        }
    }

    /// Helper: Track a packet in the provided map.
    fn track_packet(
        map: &mut BTreeMap<u32, DataPacket>,
        sequence: u32,
        payload_len: u16,
        total_length: u16,
        timestamp: SystemTime,
        flags: &TcpFlags,
    ) {
        let mut len = payload_len;
        if flags.is_syn() || flags.is_fin() {
            len += 1;
        }
        if len > 0 {
            match map.get_mut(&sequence) {
                Some(existing) => {
                    existing.retransmissions += 1;
                }
                None => {
                    let new_packet = DataPacket {
                        payload_len: len,
                        total_length,
                        sent_time: timestamp,
                        retransmissions: 0,
                        rtt: None,
                    };
                    map.insert(sequence, new_packet);
                }
            }
        }
    }

    /// Helper: Update and remove all packets in the provided map that are
    /// fully acknowledged by `ack`. Also update RTT and register the sent packet.
    fn update_acked_packets(
        map: &mut BTreeMap<u32, DataPacket>,
        ack: u32,
        ack_timestamp: SystemTime,
    ) -> Vec<DataPacket> {
        let mut acked = Vec::new();
        let mut keys_to_remove = Vec::new();

        for (&seq, sent_packet) in map.iter_mut() {
            if seq_less_equal(seq + sent_packet.payload_len as u32, ack) {

                if let Ok(rtt_duration) = ack_timestamp.duration_since(sent_packet.sent_time) {
                    println!("{:?} {}", rtt_duration, sent_packet.retransmissions);
                    sent_packet.rtt = Some(rtt_duration);
                }
                keys_to_remove.push(seq);
            } else {
                break;
            }
        }

        for seq in keys_to_remove {
            if let Some(p) = map.remove(&seq) {
                acked.push(p);
            }
        }
        acked
    }

    /// Register a packet from the stream.
    /// Outgoing non-pure-ACK packets are tracked in local_sent_packets.
    /// Incoming non-pure-ACK packets are tracked in remote_sent_packets.
    /// Pure ACKs will update the opposing map and return acknowledged packets.
    pub fn register_packet(&mut self, packet: &ParsedPacket) -> Vec<DataPacket> {
        let mut acked_packets = Vec::new();

        if let TransportPacket::TCP {
            sequence,
            acknowledgment,
            payload_len,
            flags,
            ..
        } = &packet.transport
        {
            match packet.direction {
                Direction::Outgoing => {
                    if !(flags.is_ack() && *payload_len == 0) {
                        if self.initial_sequence_local.is_none() {
                            self.initial_sequence_local = Some(*sequence);
                        }
                        Self::track_packet(
                            &mut self.local_sent_packets,
                            *sequence,
                            *payload_len,
                            packet.total_length,
                            packet.timestamp,
                            flags,
                        );
                    } else {
                        // Pure ACK from outgoing side acknowledges remote packets.
                        acked_packets = Self::update_acked_packets(
                            &mut self.remote_sent_packets,
                            *acknowledgment,
                            packet.timestamp,
                        );
                    }
                }
                Direction::Incoming => {
                    if !(flags.is_ack() && *payload_len == 0) {
                        if self.initial_sequence_remote.is_none() {
                            self.initial_sequence_remote = Some(*sequence);
                        }
                        Self::track_packet(
                            &mut self.remote_sent_packets,
                            *sequence,
                            *payload_len,
                            packet.total_length,
                            packet.timestamp,
                            flags,
                        );
                    } else {
                        // Pure ACK from incoming side acknowledges local packets.
                        acked_packets = Self::update_acked_packets(
                            &mut self.local_sent_packets,
                            *acknowledgment,
                            packet.timestamp,
                        );
                    }
                }
            }
        }
        acked_packets
    }
}

impl DefaultState for TcpTracker {
    fn default(_protocol: IpNextHeaderProtocol) -> Self {
        Self::new()
    }
    fn register_packet(&mut self, packet: &ParsedPacket) -> Vec<DataPacket> {
        self.register_packet(packet)
    }
}
