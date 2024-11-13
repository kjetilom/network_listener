use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use super::parser::{ParsedPacket, TransportPacket};
use procfs::net::{TcpState, UdpState};

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
    initial_sequence_remote: Option<u32>,
    pub last_registered: SystemTime,
    pub stats: Stats,
    total_bytes_sent: u64,
    total_bytes_acked: u64,
    start_time: SystemTime,
}

#[derive(Debug)]
pub struct UdpTracker {
    pub last_registered: SystemTime,
    pub state: Option<UdpState>,
}

/// Wrap-around aware comparison
fn seq_cmp(a: u32, b: u32) -> i32 {
    (a.wrapping_sub(b)) as i32
}

fn seq_less_equal(a: u32, b: u32) -> bool {
    seq_cmp(a, b) <= 0
}

#[derive(Debug)]
pub struct RTT {
    pub rtt: Duration,
    pub packet_size: u32,
    pub timestamp: SystemTime,
}

#[derive(Debug)]
pub struct Stats {
    pub total_retransmissions: u32,
    pub total_unique_packets: u32,
    pub rtts: Vec<RTT>,
    pub state: Option<TcpState>,
}

impl Stats {
    pub fn new() -> Self {
        Stats {
            total_retransmissions: 0,
            total_unique_packets: 0,
            rtts: Vec::new(),
            state: None,
        }
    }

    pub fn register_rtt(&mut self, rtt: RTT) {
        self.rtts.push(rtt);
    }

    pub fn register_retransmission(&mut self) {
        self.total_retransmissions += 1;
    }

    pub fn register_packet(&mut self) {
        self.total_unique_packets += 1;
    }

    pub fn set_state(&mut self, state: TcpState) {
        self.state = Some(state);
    }
}

impl TcpTracker {
    pub fn new(timestamp: SystemTime) -> Self {
        TcpTracker {
            sent_packets: BTreeMap::new(),
            initial_sequence_local: None,
            initial_sequence_remote: None,
            last_registered: timestamp,
            stats: Stats::new(),
            total_bytes_sent: 0,
            total_bytes_acked: 0,
            start_time: timestamp,
        }
    }

    pub fn handle_outgoing_packet(
        &mut self,
        packet: ParsedPacket,
    ) {
        if let TransportPacket::TCP {
            sequence,
            payload_len,
            ..
        } = &packet.transport
        {
            if packet.transport.is_syn() && !packet.transport.is_ack() {
                self.initial_sequence_local = Some(*sequence);
            }

            self.total_bytes_sent += packet.total_length as u64;

            if let Some(_initial_seq) = self.initial_sequence_local {
                let seq = *sequence;

                // Calculate the length considering SYN and FIN flags.
                let mut len = *payload_len as u32;
                if packet.transport.is_syn() || packet.transport.is_fin() {
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
    }

    pub fn handle_incoming_packet(&mut self, packet: ParsedPacket) {
        if let TransportPacket::TCP {
            acknowledgment,
            ..
        } = &packet.transport
        {
            if !packet.transport.is_ack() {
                return;
            }

            if let Some(initial_seq_local) = self.initial_sequence_local {
                let ack = acknowledgment.wrapping_sub(initial_seq_local);

                let bytes_acked = ack as u64;
                self.total_bytes_acked = bytes_acked;

                let mut keys_to_remove = Vec::new();

                for (&seq, sent_packet) in &self.sent_packets {
                    if seq_less_equal(seq + sent_packet.len - 1, ack - 1) {
                        if sent_packet.retransmissions == 0 {
                            if let Ok(rtt) = packet.timestamp.duration_since(sent_packet.sent_time) {
                                let rtt_entry = RTT {
                                    rtt,
                                    packet_size: sent_packet.total_packet_size,
                                    timestamp: packet.timestamp,
                                };
                                self.stats.register_rtt(rtt_entry);
                            }
                        }
                        keys_to_remove.push(seq);
                    }
                }

                for seq in keys_to_remove {
                    self.sent_packets.remove(&seq);
                }
            }
        }
    }

    pub fn get_bandwidth(&self) -> Option<f64> {
        if let Ok(duration) = self.last_registered.duration_since(self.start_time) {
            let seconds = duration.as_secs_f64();
            if seconds > 0.0 {
                let bandwidth = (self.total_bytes_acked as f64) / seconds;
                return Some(bandwidth);
            }
        }
        None
    }
}
