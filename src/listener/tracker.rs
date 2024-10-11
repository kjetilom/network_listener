use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use super::parser::{ParsedPacket, Timeval};

pub(crate) static TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug)]
pub struct PacketTracker {
    pub sent_packets: HashMap<u32, Timeval>, // Keyed by TCP sequence number
    pub processed_acks: HashSet<u32>,
    timeout: Duration,
}

impl PacketTracker {
    pub fn new(timeout: Duration) -> Self {
        PacketTracker {
            sent_packets: HashMap::new(),
            processed_acks: HashSet::new(),
            timeout,
        }
    }

    /*
     * Records a sent packet's sequence number and timestamp.
     */
    pub fn record_sent(&mut self, packet: &ParsedPacket) {
        self.sent_packets.insert(packet.sequence, packet.timestamp);
    }

    /* Records an acknowledgment number and calculates RTT if possible.
     *
     * Returns `Some(Duration)` if RTT can be calculated, otherwise `None`.
     */
    pub fn record_ack(&mut self, packet: &ParsedPacket) -> Option<Duration> {
        // if self.processed_acks.contains(&packet.acknowledgment) {
        //     return None;
        // }
        self.processed_acks.insert(packet.acknowledgment);
        if let Some(sent_time) = self.sent_packets.remove(&(packet.acknowledgment - 1)) {
            Some(packet.timestamp - sent_time)
        } else {
            None
        }
    }

    pub fn acknowledge(&mut self, packet: &ParsedPacket) -> Option<Duration> {
        // Find all sequence numbers less than ack_number
        let mut rtt = None;
        let acknowledged_sequences: Vec<u32> = self.sent_packets
            .keys()
            .filter(|&&seq| seq <= packet.acknowledgment)
            .cloned()
            .collect();

        for seq in acknowledged_sequences {
            if let Some(stv) = self.sent_packets.remove(&seq) {
                let current_rtt = packet.timestamp - stv;
                // You can choose to store the RTTs or return the latest one
                rtt = Some(current_rtt);
            }
        }
        rtt
    }

    pub fn cleanup(&mut self) {
        let now = Instant::now();

    }
}
