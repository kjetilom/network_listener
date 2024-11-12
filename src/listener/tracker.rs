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
    sent_packets: BTreeMap<u32, SentPacket>, // Keyed by absolute sequence number
    initial_sequence_local: Option<u32>,
    initial_sequence_remote: Option<u32>,
    pub last_registered: SystemTime,
    pub state: Option<TcpState>,
    total_retransmissions: u32,
    total_unique_packets: u32,
    pub rtt_to_size: Vec<(u32, Duration)>,
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

pub struct Stats {
    pub total_retransmissions: u32,
    pub total_unique_packets: u32,
    pub rtt_to_size: Vec<(u32, Duration)>,
    pub state: Option<TcpState>,

}

impl TcpTracker {
    pub fn new() -> Self {
        TcpTracker {
            sent_packets: BTreeMap::new(),
            initial_sequence_local: None,
            initial_sequence_remote: None,
            last_registered: SystemTime::now(),
            state: None,
            total_retransmissions: 0,
            total_unique_packets: 0,
            rtt_to_size: Vec::new(),
        }
    }

    pub fn handle_outgoing_packet(
        &mut self,
        packet: &ParsedPacket,
        is_syn: bool,
        is_ack: bool,
    ) {
        if let TransportPacket::TCP {
            sequence,
            payload_len,
            ..
        } = &packet.transport
        {
            if is_syn && !is_ack {
                self.initial_sequence_local = Some(*sequence);
            }

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
                        self.total_retransmissions += 1;
                        // Do not update sent_time to keep the original send time (Karn's Algorithm).
                    } else {
                        // New packet sent.
                        let sent_packet = SentPacket {
                            len,
                            sent_time: packet.timestamp,
                            retransmissions: 0,
                            total_packet_size: packet.total_length,
                        };
                        self.total_unique_packets += 1;

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

    pub fn handle_incoming_packet(
        &mut self,
        packet: &ParsedPacket,
        is_syn: bool,
        is_ack: bool,
    ) -> Option<Duration> {
        if let TransportPacket::TCP {
            sequence,
            acknowledgment,
            ..
        } = &packet.transport
        {
            if is_syn {
                // SYN received
                self.initial_sequence_remote = Some(*sequence);
            }

            if is_ack {
                if let Some(_initial_seq_local) = self.initial_sequence_local {
                    let ack = *acknowledgment;

                    let mut rtts = Vec::new();
                    let mut keys_to_remove = Vec::new();

                    for (&seq, sent_packet) in self.sent_packets.iter() {
                        // Check if the packet is fully acknowledged.
                        if seq_less_equal(
                            seq.wrapping_add(sent_packet.len - 1),
                            ack.wrapping_sub(1),
                        ) {
                            if sent_packet.retransmissions == 0 {
                                // Compute RTT for packets not retransmitted.
                                if let Ok(rtt) = packet.timestamp.duration_since(sent_packet.sent_time) {
                                    rtts.push(rtt);
                                }
                            }
                            keys_to_remove.push(seq);
                        }
                    }
                    if keys_to_remove.len() == 1 && rtts.len() == 1 {
                        let seq = keys_to_remove[0];
                        let rtt = rtts[0];
                        if let Some(sent_packet) = self.sent_packets.get(&seq) {
                            let rtt_to_size = (sent_packet.total_packet_size, rtt);
                            self.rtt_to_size.push(rtt_to_size);
                        }
                    }

                    // Remove acknowledged packets from sent_packets.
                    for seq in keys_to_remove {

                        self.sent_packets.remove(&seq);
                    }

                    // Return the most recent RTT measurement.
                    if let Some(rtt) = rtts.last() {
                        return Some(*rtt);
                    }
                }
            }
        }
        None
    }

    /// Get the total number of retransmissions.
    pub fn get_retransmission_count(&self) -> u32 {
        self.total_retransmissions
    }
}
