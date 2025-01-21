use std::time::SystemTime;

use crate::listener::{packet::packet_builder::ParsedPacket, stream_manager::StreamManager};



/// What is a link?
/// Two devices connected by a physical or virtual medium. (In this case.)
/// The link module is responsible for managing the link between the two devices.
/// Specifically: It manages the streams between two devices.
///
/// A link will have a set of streams.
/// Note: ICMP "streams" will be tracked in one stream since they are not connection oriented.
struct Link {
    streams: StreamManager,
    last_recorded: SystemTime, // Timestamp of last captured packet
    calculated_throughput: f64, // Calculated throughput of the link
}

impl Link {
    /// Create a new link with an empty stream manager.
    pub fn new() -> Self {
        Link {
            streams: StreamManager::default(),
            last_recorded: SystemTime::now(),
            calculated_throughput: 0.0,
        }
    }

    /// Record a packet on the link.
    pub fn record_packet(&mut self, packet: ParsedPacket) {

    }

}