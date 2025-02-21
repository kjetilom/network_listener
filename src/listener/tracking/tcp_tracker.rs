use std::collections::BTreeMap;
use std::time::SystemTime;

use pnet::packet::ip::IpNextHeaderProtocol;

use crate::{
    tracker::DefaultState, Direction, PacketType, ParsedPacket, TcpFlags, TransportPacket
};

/// Wrap-around aware sequence comparison.
fn seq_cmp(a: u32, b: u32) -> i32 {
    a.wrapping_sub(b) as i32
}
fn seq_less_equal(a: u32, b: u32) -> bool {
    seq_cmp(a, b) <= 0
}

/// TCP tracker which now tracks packets from both directions.
/// Outgoing packets (from us) are stored in `local_sent_packets` and
/// packets from the remote side are stored in `remote_sent_packets`.
#[derive(Debug)]
pub struct TcpTracker {
    local_sent_packets: BTreeMap<u32, PacketType>,
    remote_sent_packets: BTreeMap<u32, PacketType>,
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
        map: &mut BTreeMap<u32, PacketType>,
        sequence: u32,
        packet: PacketType,
        flags: &TcpFlags,
    ) {
        let mut len = packet.payload_len;
        if flags.is_syn() || flags.is_fin() {
            len += 1;
        }
        if len > 0 {
            match map.get_mut(&sequence) {
                Some(existing) => {
                    existing.retransmissions += 1;
                    // If we don't do this we will calculate a way too high RTT
                    existing.sent_time = packet.sent_time;
                }
                None => {
                    map.insert(sequence, packet);
                }
            }
        }
    }

    /// Helper: Update and remove all packets in the provided map that are
    /// fully acknowledged by `ack`. Also update RTT and register the sent packet.
    fn update_acked_packets(
        map: &mut BTreeMap<u32, PacketType>,
        ack: u32,
        ack_timestamp: SystemTime,
    ) -> Vec<PacketType> {
        let mut acked = Vec::new();
        let mut keys_to_remove = Vec::new();
        for (&seq, sent_packet) in map.iter_mut() {
            if seq_less_equal(seq + sent_packet.payload_len as u32, ack) {
                if let Ok(rtt_duration) = ack_timestamp.duration_since(sent_packet.sent_time) {
                    sent_packet.rtt = Some(rtt_duration);
                }
                keys_to_remove.push(seq);
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
    pub fn register_packet(&mut self, packet: &ParsedPacket) -> Vec<PacketType> {
        let mut acked_packets = Vec::new();

        if let TransportPacket::TCP {
            sequence,
            acknowledgment,
            payload_len,
            flags,
            ..
        } = &packet.transport
        {
            let pkt = PacketType::from_packet(packet);
            match packet.direction {
                Direction::Outgoing => {
                    if flags.is_ack() && *payload_len == 0 {
                        // Pure ACK from outgoing side acknowledges remote packets.
                        acked_packets = Self::update_acked_packets(
                            &mut self.remote_sent_packets,
                            *acknowledgment,
                            packet.timestamp,
                        );
                    } else {
                        if self.initial_sequence_local.is_none() {
                            self.initial_sequence_local = Some(*sequence);
                        }
                        Self::track_packet(
                            &mut self.local_sent_packets,
                            *sequence,
                            pkt,
                            flags,
                        );
                    }
                }
                Direction::Incoming => {
                    if flags.is_ack() && *payload_len == 0 {
                        // Pure ACK from incoming side acknowledges local packets.
                        acked_packets = Self::update_acked_packets(
                            &mut self.local_sent_packets,
                            *acknowledgment,
                            packet.timestamp,
                        );
                    } else {
                        if self.initial_sequence_remote.is_none() {
                            self.initial_sequence_remote = Some(*sequence);
                        }
                        Self::track_packet(
                            &mut self.remote_sent_packets,
                            *sequence,
                            pkt,
                            flags,
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
    fn register_packet(&mut self, packet: &ParsedPacket) -> Vec<PacketType> {
        self.register_packet(packet)
    }
}
