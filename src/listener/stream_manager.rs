use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

use super::parser::{ParsedPacket, TransportPacket};
use super::stream_id::TcpStreamId;
use super::tracker::PacketTracker;

#[derive(Debug)]
pub struct TcpStreamManager {
    streams: HashMap<TcpStreamId, PacketTracker>,
    timeout: Duration,
}

impl TcpStreamManager {
    pub fn new(timeout: Duration) -> Self {
        TcpStreamManager {
            streams: HashMap::new(),
            timeout,
        }
    }

    pub fn record_sent_packet(&mut self, packet: &ParsedPacket, own_ip: IpAddr) {
        if let TransportPacket::TCP { flags, sequence, .. } = packet.transport {
            let stream_id = TcpStreamId::from(packet);
            //let is_syn = packet.flags & 0x02 != 0;
            let is_ack = flags & 0x10 != 0;

            if !is_ack && packet.src_ip == own_ip {
                let tracker = self.streams.entry(stream_id)
                    .or_insert_with(|| PacketTracker::new(self.timeout));
                tracker.record_sent(sequence);
            }
        }
    }

    pub fn record_ack_packet(&mut self, packet: &ParsedPacket) -> Option<Duration> {
        if let TransportPacket::TCP { flags, acknowledgment, .. } = packet.transport {
            let is_ack = flags & 0x10 != 0;

            if is_ack {
                // If the packet is an ACK, reverse the stream ID
                let stream_id = TcpStreamId::from_reversed(&packet);

                if let Some(tracker) = self.streams.get_mut(&stream_id) {
                    tracker.record_ack(acknowledgment)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Cleans up all streams by removing probes that have timed out.
    pub fn cleanup(&mut self) {
        self.streams.values_mut().for_each(|tracker| tracker.cleanup());
        // Optionally, remove streams with no outstanding probes
        self.streams.retain(|_, tracker| !tracker.sent_packets.is_empty());
    }
}
