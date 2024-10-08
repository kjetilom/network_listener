use std::collections::HashMap;
use std::time::{Instant, Duration};

pub struct PacketTracker {
    sent_packets: HashMap<u32, Instant>, // Keyed by TCP sequence number
}

impl PacketTracker {
    pub fn new() -> Self {
        PacketTracker {
            sent_packets: HashMap::new(),
        }
    }

    /// Records a sent packet's sequence number and timestamp.
    pub fn record_sent(&mut self, sequence: u32) {
        self.sent_packets.insert(sequence, Instant::now());
    }

    /// Records an acknowledgment number and calculates RTT if possible.
    ///
    /// Returns `Some(Duration)` if RTT can be calculated, otherwise `None`.
    pub fn record_ack(&mut self, acknowledgment: u32) -> Option<Duration> {
        if acknowledgment > 0 {
            let expected_seq = acknowledgment - 1;
            if let Some(sent_time) = self.sent_packets.remove(&expected_seq) {
                return Some(sent_time.elapsed());
            } else {
                None
            }
        } else {
            None
        }
    }
}