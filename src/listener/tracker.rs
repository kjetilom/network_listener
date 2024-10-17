use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use super::parser::{ParsedPacket, TransportPacket};

// TCP connection states
#[derive(Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Closed,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    LastAck,
    TimeWait,
    Closing,
    Reset,
    // Custom state since we start tracking a connection at any point
    Unknown,
}


/// Tracks TCP streams and their state.
#[derive(Debug)]
pub struct PacketTracker {
    pub sent_packets: HashMap<u32, SystemTime>, // Keyed by TCP sequence number
    pub initial_sequence_local: Option<u32>,
    pub initial_sequence_remote: Option<u32>,
    pub last_registered: SystemTime,
    pub timeout: Duration,
    pub state: ConnectionState,
}

impl PacketTracker {
    pub fn new(timeout: Duration) -> Self {
        PacketTracker {
            sent_packets: HashMap::new(),
            initial_sequence_local: None,
            initial_sequence_remote: None,
            last_registered: SystemTime::now(),
            timeout,
            state: ConnectionState::Unknown,
        }
    }

    pub fn handle_outgoing_packet(&mut self, packet: &ParsedPacket, is_syn: bool, is_ack: bool, is_fin: bool, is_rst: bool) {
        if let TransportPacket::TCP { sequence, .. } = &packet.transport {
            if is_syn && !is_ack {
                self.state = ConnectionState::SynSent;
                self.initial_sequence_local = Some(*sequence);
            }

            if is_syn && is_ack {
                self.state = ConnectionState::Established;
            }

            if is_fin {
                self.state = ConnectionState::FinWait1;
            }

            if is_rst {
                self.state = ConnectionState::Reset;
            }

            if let Some(initial_seq) = self.initial_sequence_local {
                let relative_seq = sequence.wrapping_sub(initial_seq);
                self.sent_packets.insert(relative_seq, packet.timestamp);
            } else {
                self.initial_sequence_local = Some(*sequence);
            }
        }
    }

    pub fn handle_incoming_packet(&mut self, packet: &ParsedPacket, is_syn: bool, is_ack: bool, is_fin: bool, is_rst: bool) -> Option<Duration> {
        if let TransportPacket::TCP { sequence, acknowledgment, .. } = &packet.transport {
            if is_syn && !is_ack { // SYN received
                self.state = ConnectionState::SynReceived;
                self.initial_sequence_remote = Some(*sequence);
            } else if is_syn && is_ack { // SYN-ACK received
                self.state = ConnectionState::SynReceived;
                self.initial_sequence_remote = Some(*sequence);
            }

            if (self.state == ConnectionState::SynSent || self.state == ConnectionState::SynReceived) && is_ack {
                self.state = ConnectionState::Established;
            }

            if is_fin {
                self.state = ConnectionState::CloseWait;
            }

            if is_rst {
                self.state = ConnectionState::Reset;
            }

            if is_ack {
                if let Some(initial_seq_local) = self.initial_sequence_local {
                    let relative_ack = acknowledgment.wrapping_sub(initial_seq_local);
                    if let Some(sent_time) = self.sent_packets.remove(&relative_ack) {
                        return Some(sent_time.elapsed().unwrap_or_default());
                    }
                }
            }
        }
        None
    }

    pub fn cleanup(&mut self) {
        let timeout = self.timeout;
        self.sent_packets.retain(|_, &mut timestamp| {
            timestamp.elapsed().unwrap() < timeout
        });
    }
}
