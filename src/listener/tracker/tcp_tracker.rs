use std::collections::{BTreeMap, HashSet, VecDeque};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pnet::packet::ip::IpNextHeaderProtocol;
use procfs::net::TcpState;

use super::super::packet::{
    direction::Direction,
    packet_builder::ParsedPacket,
    transport_packet::{TcpFlags, TransportPacket},
};

use super::link::DataPoint;
use super::tracker::{DefaultState, SentPacket};

/// Wrap-around aware comparison
fn seq_cmp(a: u32, b: u32) -> i32 {
    a.wrapping_sub(b) as i32
}

fn seq_less_equal(a: u32, b: u32) -> bool {
    seq_cmp(a, b) <= 0
}

#[derive(Debug)]
pub struct TcpStats {
    pub total_retransmissions: u32,
    pub total_unique_packets: u32,
    pub recv: VecDeque<SentPacket>,
    pub sent: VecDeque<SentPacket>,
    pub received_seqs: HashSet<u32>,
    pub state: Option<TcpState>,
    pub initial_rtt: Option<Duration>,
}

impl TcpStats {
    pub fn new() -> Self {
        TcpStats {
            total_retransmissions: 0,
            total_unique_packets: 0,
            recv: VecDeque::with_capacity(1000),
            sent: VecDeque::with_capacity(1000),
            received_seqs: HashSet::new(),
            state: None,
            initial_rtt: None,
        }
    }

    pub fn register_data_received(&mut self, p: SentPacket, seq: &u32) {
        if let Some(seq) = self.received_seqs.get(seq) {
            dbg!("RETRANSMISSION", seq);
            // Now look at the recv buffer. Iterate through the buffer and print each sent time relative to the current packet.
            let first_sent_time = self.recv.front().unwrap().sent_time;
            for packet in self.recv.iter().rev() {
                let time_diff = packet.sent_time.duration_since(first_sent_time);
                if let Ok(time_diff) = time_diff {
                    dbg!(time_diff.as_micros(), packet.len);
                }
            }
        }
        // Assumption 1: If a retransmission has occurred, the link is possibly congested.
        self.received_seqs.insert(*seq);
        self.recv.push_back(p);

    }

    pub fn register_data_sent(&mut self, p: SentPacket) {
        self.sent.push_back(p);
    }

    pub fn estimate_available_bandwidth(&self) -> Option<f64> {
        // Try to find a section of RTT measurements where there is an increase in RTT
        let mut packets_with_rtt: Vec<&SentPacket> = self.sent
            .iter()
            .filter(|p| p.rtt.is_some())
            .collect();

        Some(0.0)
    }

    pub fn estimate_bandwidth(&self) -> Option<f64> {
        // Gather all packets with a valid RTT
        let mut acked_packets: Vec<&SentPacket> = self.sent
            .iter()
            .filter(|p| p.rtt.is_some())
            .collect();

        // If fewer than 2 packets have RTT, we can't form a time interval
        if acked_packets.len() < 2 {
            return None;
        }

        // Sort by ACK reception time (sent_time + rtt)
        acked_packets.sort_by_key(|p| {
            let ack_time = p.sent_time + p.rtt.unwrap();
            ack_time
        });

        // Earliest ACK time
        let first_ack_time = acked_packets[0].sent_time + acked_packets[0].rtt.unwrap();
        // Latest ACK time
        let last_ack_time = acked_packets.last().unwrap().sent_time
                            + acked_packets.last().unwrap().rtt.unwrap();

        // Convert times to seconds since epoch for a continuous timescale
        let first_ack_secs = first_ack_time.duration_since(UNIX_EPOCH).ok()?.as_secs_f64();
        let last_ack_secs = last_ack_time.duration_since(UNIX_EPOCH).ok()?.as_secs_f64();

        let elapsed = last_ack_secs - first_ack_secs;
        if elapsed <= 0.0 {
            return None;
        }

        // Sum up the byte lengths of all acked packets
        let total_bytes: u64 = acked_packets.iter().map(|p| p.len as u64).sum();

        // Bandwidth in bytes per second
        let bandwidth_bps = total_bytes as f64 / elapsed;

        Some(bandwidth_bps)
    }

    pub fn consume_to_vec(&mut self) -> Vec<DataPoint> {
        let ret = self.sent
            .iter()
            .filter_map(|p| {
                <Option<Duration> as Clone>::clone(&p.rtt).map(|rtt| {
                    DataPoint::new(
                        p.len as u16,
                        p.sent_time,
                        Some(rtt.as_micros() as u32),
                    )
                })
            })
            .collect();
        self.sent.clear();
        ret
    }
}


#[derive(Debug)]
pub struct TcpTracker {
    sent_packets: BTreeMap<u32, SentPacket>,
    initial_sequence_local: Option<u32>,
    pub stats: TcpStats,
    total_bytes_sent: u64,
    total_bytes_acked: u64,
    pub data: Vec<DataPoint>,
}

impl TcpTracker {
    pub fn new() -> Self {
        TcpTracker {
            sent_packets: BTreeMap::new(),
            initial_sequence_local: None,
            stats: TcpStats::new(),
            total_bytes_sent: 0,
            total_bytes_acked: 0,
            data: Vec::new(),
        }
    }

    pub fn store_data(&mut self) {
        self.data.extend(self.stats.consume_to_vec());
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

                        self.stats.register_data_received(SentPacket {
                            len: *payload_len as u32,
                            sent_time: packet.timestamp,
                            retransmissions: 0,
                            rtt: None,
                        },
                        sequence,
                    );
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
                        dbg!("RETRANSMISSION", sequence);
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
                // If packet is fully acked, calculate RTT
                if let Ok(rtt_duration) = ack_timestamp.duration_since(sent_packet.sent_time) {
                    sent_packet.rtt = Some(rtt_duration);
                    // Optionally store first RTT for future usage
                    if self.stats.initial_rtt.is_none() {
                        self.stats.initial_rtt = sent_packet.rtt.clone();
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

    fn extract_data(&mut self) -> Vec<DataPoint> {
        self.data.clone()
    }
}
