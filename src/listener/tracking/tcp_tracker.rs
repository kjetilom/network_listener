use std::collections::{BTreeMap, HashSet, VecDeque};
use std::time::{Duration, SystemTime};

use pnet::packet::ip::IpNextHeaderProtocol;
use procfs::net::TcpState;

use crate::{
    Direction, ParsedPacket, {TcpFlags, TransportPacket},
};

use super::tracker::{DefaultState, RegPkt};

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
    pub rts: u32,
    pub recv: VecDeque<RegPkt>,
    pub sent: VecDeque<RegPkt>,
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
            sent: VecDeque::with_capacity(1000),
            received_seqs: HashSet::new(),
            state: None,
            smoothed_rtt: None,
            prev_smoothed_rtt: None,
        }
    }

    pub fn register_data_received(&mut self, mut p: RegPkt, seq: &u32) {
        if self.received_seqs.contains(seq) {
            p.retransmissions += 1;
        }
        self.received_seqs.insert(*seq);
        self.recv.push_back(p);
    }

    pub fn register_data_sent(&mut self, p: RegPkt) {
        if let Some(rtt) = p.rtt {
            self.update_rtt(rtt);
        }
        self.sent.push_back(p);
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
    sent_packets: BTreeMap<u32, RegPkt>,
    initial_sequence_local: Option<u32>,
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
                            RegPkt {
                                len: *payload_len,
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
            let mut len = payload_len;
            if flags.is_syn() || flags.is_fin() {
                len += 1;
            }

            if len > 0 {
                match self.sent_packets.get_mut(&sequence) {
                    Some(existing) => {
                        existing.retransmissions += 1;
                    }
                    None => {
                        let new_packet = RegPkt {
                            len,
                            sent_time: timestamp,
                            retransmissions: 0,
                            rtt: None,
                        };
                        self.sent_packets.insert(sequence, new_packet);
                    }
                }
            }
        }
    }

    fn update_acked_packets(&mut self, ack: u32, ack_timestamp: SystemTime) {
        let mut keys_to_remove = Vec::new();

        for (&seq, sent_packet) in self.sent_packets.iter_mut() {
            if seq_less_equal(seq + sent_packet.len as u32, ack) {
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


#[cfg(test)]
mod tests {
    use pnet::util::MacAddr;

    use super::*;
    use crate::Direction;
    use crate::TransportPacket;
    use std::net::Ipv4Addr;
    use std::time::Duration;

    #[test]
    fn test_tcp_tracker() {
        let mut tracker = TcpTracker::new();
        let timestamp = SystemTime::now();
        let seq = 0;
        let ack = 0;
        let payload_len = 10;
        let flags = TcpFlags::new(TcpFlags::ACK);

        let ack_timestamp = timestamp + Duration::from_secs(1);
        let packet = ParsedPacket {
            src_ip: std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            dst_ip: std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)),
            src_mac: MacAddr::new(0, 0, 0, 0, 0, 0),
            dst_mac: MacAddr::new(0, 0, 0, 0, 0, 0),
            intercepted: false,
            timestamp: ack_timestamp,
            transport: TransportPacket::TCP {
                src_port: 1,
                dst_port: 10,
                sequence: seq,
                acknowledgment: 0,
                payload_len,
                flags: TcpFlags::new(0),
                options: crate::TcpOptions { tsval: None, tsecr: None, scale: None, mss: None },
                window_size: 0,
            },
            direction: Direction::Outgoing,
            total_length: 0,
        };

        tracker.register_packet(&packet);

        assert_eq!(tracker.stats.rts, 0);
        assert_eq!(tracker.sent_packets.len(), 1);
        assert_eq!(tracker.stats.recv.len(), 0);

        let ack_timestamp = timestamp + Duration::from_secs(1);
        let ack_packet = ParsedPacket {
            src_ip: std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            dst_ip: std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)),
            src_mac: MacAddr::new(0, 0, 0, 0, 0, 0),
            dst_mac: MacAddr::new(0, 0, 0, 0, 0, 0),
            intercepted: false,
            timestamp: ack_timestamp,
            transport: TransportPacket::TCP {
                src_port: 1,
                dst_port: 10,
                sequence: ack,
                acknowledgment: seq + payload_len as u32,
                payload_len: 0,
                flags: flags,
                options: crate::TcpOptions { tsval: None, tsecr: None, scale: None, mss: None },
                window_size: 0,
            },
            direction: Direction::Incoming,
            total_length: 50,
        };

        tracker.register_packet(&ack_packet);

        assert_eq!(tracker.stats.sent.len(), 1);
        assert_eq!(tracker.stats.recv.len(), 0);
    }
}