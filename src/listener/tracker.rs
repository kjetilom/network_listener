use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use super::parser::{ParsedPacket, TransportPacket};
use procfs::net::TcpState;

/// Tracks TCP streams and their state.
#[derive(Debug)]
pub struct PacketTracker {
    pub sent_packets: BTreeMap<u32, Vec<SystemTime>>, // Keyed by relative sequence number
    pub initial_sequence_local: Option<u32>,
    pub initial_sequence_remote: Option<u32>,
    pub last_registered: SystemTime,
    pub timeout: Duration,
    pub state: Option<TcpState>,
    pub next_expected_seq_out: Option<u32>,
    pub total_retransmissions: u32,
}

// fn seq_less(a: u32, b: u32) -> bool {
//     ((a as i32).wrapping_sub(b as i32)) < 0
// }

fn seq_less_equal(a: u32, b: u32) -> bool {
    ((a as i32).wrapping_sub(b as i32)) <= 0
}

impl PacketTracker {
    pub fn new(timeout: Duration) -> Self {
        PacketTracker {
            sent_packets: BTreeMap::new(),
            initial_sequence_local: None,
            initial_sequence_remote: None,
            last_registered: SystemTime::now(),
            timeout,
            state: None,
            next_expected_seq_out: None,
            total_retransmissions: 0,
        }
    }

    pub fn handle_outgoing_packet(
        &mut self,
        packet: &ParsedPacket,
        is_syn: bool,
        is_ack: bool,
    ) -> (){
        if let TransportPacket::TCP {
            sequence,
            payload_len,
            ..
        } = &packet.transport {
            if is_syn && !is_ack {
                self.initial_sequence_local = Some(*sequence);
            }

            //println!("Payload_size: {}, Seq: {}", payload_len, sequence);


            if let Some(initial_seq) = self.initial_sequence_local {
                let relative_seq = sequence.wrapping_sub(initial_seq);
                if let Some(timestamps) = self.sent_packets.get_mut(&relative_seq) {
                    timestamps.push(packet.timestamp);
                } else {
                    self.sent_packets.insert(relative_seq, vec![packet.timestamp]);
                }
            } else {
                // Since we don't know the initial sequence number,
                // we'll just count the first packet as the initial one
                self.initial_sequence_local = Some(*sequence);
            }
            self.next_expected_seq_out = Some(sequence.wrapping_add(*payload_len as u32));
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
        } = &packet.transport {
            if is_syn {
                // SYN received
                self.initial_sequence_remote = Some(*sequence);
            }

            if is_ack {
                if let Some(initial_seq_local) = self.initial_sequence_local {
                    let relative_ack = acknowledgment.wrapping_sub(initial_seq_local);

                    // Collect RTTs
                    let mut rtts = Vec::new();
                    let mut keys_to_remove = Vec::new();

                    for (&seq, timestamps) in self.sent_packets.iter() {
                        if seq_less_equal(seq, relative_ack) {
                            if timestamps.len() == 1 {
                                let sent_time = timestamps[0];
                                let rtt = packet.timestamp.duration_since(sent_time).unwrap_or_default();
                                rtts.push(rtt);
                            } else {
                                //println!("Multiple timestamps for seq {}", seq);
                                //return None;
                            }
                            keys_to_remove.push(seq);
                        } else {
                            break;
                        }
                    }

                    // Remove acknowledged packets from sent_packets
                    for seq in keys_to_remove {
                        self.sent_packets.remove(&seq);
                    }

                    // Return the most recent RTT measurement
                    if let Some(rtt) = rtts.last() {
                        return Some(*rtt);
                    }
                }
            }
        }
        None
    }

    pub fn cleanup(&mut self) {
        let timeout = self.timeout;
        self.sent_packets.retain(|_, timestamps| {
            if let Some(last_sent_time) = timestamps.last() {
                last_sent_time.elapsed().unwrap() < timeout
            } else {
                false
            }
        });
    }

}
