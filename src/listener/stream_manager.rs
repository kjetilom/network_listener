use std::collections::HashMap;
use std::time::Duration;

use log::info;

use super::traffic_analyzer::ParsedPacket;
use super::stream_id::{Connection};
use super::traffic_analyzer::Direction;
use super::tracker::PacketTracker;

pub struct StreamManager {
    streams: HashMap<Connection, PacketTracker>,
}

#[derive(Debug)]
pub struct TcpStreamManager {
    streams: HashMap<Connection, PacketTracker>,
    timeout: Duration,
}

#[derive(Debug)]
pub struct UDPStreamManager {
    streams: HashMap<Connection, PacketTracker>,
    timeout: Duration,
}

impl TcpStreamManager {
    pub fn new(timeout: Duration) -> Self {
        TcpStreamManager {
            streams: HashMap::new(),
            timeout,
        }
    }

    pub fn record_sent(&mut self, packet: &ParsedPacket) {
        let tcp_packet = match packet.as_tcp() {
            Some(packet) => packet,
            None => panic!("Attempted to record a non-TCP packet in a TCP stream manager"),
        };
        let connection = match packet.connection() {
            Some(connection) => connection,
            None => panic!("Attempted to record a packet without a connection"),
        };
        //let is_syn = packet.flags & 0x02 != 0;
        let is_ack = tcp_packet.get_flags() & 0x10 != 0;

        let tracker = match self.streams.get_mut(&connection) {
            Some(tracker) => tracker,
            None => {
                let tracker = PacketTracker::new(self.timeout);
                self.streams.insert(connection.clone(), tracker);
                info!("New connection: {:?}", connection);
                self.streams.get_mut(&connection).unwrap() // Safe!
            }
        };
        if !is_ack {
            tracker.record_sent(packet);
        }
    }

    pub fn record_ack(&mut self, packet: &ParsedPacket) -> Option<Duration> {
        let tcp_packet = match packet.as_tcp() {
            Some(packet) => packet,
            None => panic!("Attempted to record a non-TCP packet in a TCP stream manager"),
        };
        let connection = match packet.connection() {
            Some(connection) => connection,
            None => panic!("Attempted to record a packet without a connection"),
        };
        let is_ack = tcp_packet.get_flags() & 0x10 != 0;

        if is_ack {
            if let Some(tracker) = self.streams.get_mut(&connection) {
                tracker.record_ack(packet)
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
