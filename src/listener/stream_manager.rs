use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use super::parser::{ParsedPacket, TransportPacket};
use super::stream_id::TcpStreamId;
use super::tracker::{PacketTracker, ConnectionState};


#[derive(Debug)]
pub struct TcpStreamManager {
    streams: HashMap<TcpStreamId, PacketTracker>,
    timeout: Duration,
    last_cleanup: Instant,
}


#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TcpFlags;

impl TcpStreamManager {
    pub fn new(timeout: Duration) -> Self {
        TcpStreamManager {
            streams: HashMap::new(),
            timeout,
            last_cleanup: Instant::now(),
        }
    }

    pub fn record_packet(&mut self, packet: &ParsedPacket, own_ip: IpAddr) -> Option<Duration> {
        if let TransportPacket::TCP { flags, .. } = &packet.transport {
            let is_syn = flags & 0x02 != 0;
            let is_fin = flags & 0x01 != 0;
            let is_rst = flags & 0x04 != 0;
            let is_ack = flags & 0x10 != 0;

            if self.last_cleanup.elapsed() > super::Settings::CLEANUP_INTERVAL {
                for (stream_id, tracker) in self.streams.iter() {
                    println!("{}, State: {:?}, Elapsed {:?}", stream_id, tracker.state, tracker.last_registered.elapsed());
                }
                self.cleanup();
                self.last_cleanup = Instant::now();
            }

            let stream_id = TcpStreamId::from(packet, own_ip);

            let tracker = self.streams.entry(stream_id)
                .or_insert_with(|| PacketTracker::new(self.timeout));

            tracker.last_registered = packet.timestamp;

            if packet.src_ip == own_ip {
                // Handle packets sent from own IP
                tracker.handle_outgoing_packet(packet, is_syn, is_ack, is_fin, is_rst);
            } else {
                // Handle packets received by own IP
                return tracker.handle_incoming_packet(packet, is_syn, is_ack, is_fin, is_rst);
            }
        }
        None
    }

    /// Cleans up all streams by removing probes that have timed out.
    pub fn cleanup(&mut self) {
        let curlen = self.streams.len();
        self.streams.retain(|_, tracker| {
            tracker.cleanup();
            !matches!(tracker.state, ConnectionState::Closed | ConnectionState::Reset)
                || !tracker.sent_packets.is_empty()
        });
        if curlen != self.streams.len() {
            println!("Cleaned up {} streams", curlen - self.streams.len());
        }
    }
}
