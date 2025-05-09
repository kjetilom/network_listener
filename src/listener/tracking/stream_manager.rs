use crate::{
    stream_id::StreamKey,
    tracker::{Tracker, TrackerState},
    PacketRegistry, ParsedPacket,
};
use pnet::packet::ip::IpNextHeaderProtocol;
use std::collections::HashMap;
use tokio::time::Instant;

/// Manages active transport streams, tracking their packet bursts and throughput.
///
/// Maintains separate registries for sent and received packets, and records
/// Has support for peridic iperf measurements, but this is deactivated.
#[derive(Debug)]
pub struct StreamManager {
    /// HashMap for all streams
    streams: HashMap<StreamKey, Tracker<TrackerState>>,
    /// Registry for outgoing streams (Including incoming acks).
    pub sent: PacketRegistry,
    /// Registry for streams from other nodes.
    pub received: PacketRegistry,
    /// TCP throughput in bytes per second.
    tcp_thput: f64,
    /// Last time iperf was run.
    pub last_iperf: Option<Instant>,
    /// Total bytes sent.
    bytes_sent: u32,
    /// Total bytes received.
    bytes_received: u32,
}

impl StreamManager {
    /// Create a new `StreamManager` with empty registries and zeroed counters.
    pub fn default() -> Self {
        StreamManager {
            streams: HashMap::new(),
            sent: PacketRegistry::new(),
            received: PacketRegistry::new(),
            tcp_thput: 0.0,
            last_iperf: None,
            bytes_sent: 0,
            bytes_received: 0,
        }
    }

    /// Record a new iperf throughput result (in bits per second).
    ///
    /// Updates `tcp_thput` and stamps the current instant.
    pub fn record_iperf_result(&mut self, bps: f64, _stream: Option<&crate::IperfStream>) {
        // Check if in out is very different
        self.last_iperf = Some(Instant::now());
        self.tcp_thput = bps;
    }

    /// Return the most recent TCP throughput if the last measurement is older
    /// If iperf is not used, this will always return 0.0.
    /// than the configured measurement window; otherwise return 0.0.
    pub fn tcp_thput(&self) -> f64 {
        if let Some(last_iperf) = self.last_iperf {
            if last_iperf.elapsed() > crate::CONFIG.client.measurement_window {
                return self.tcp_thput;
            }
        }
        return 0.0;
    }

    /// Process a parsed packet: updates byte counters, registers bursts,
    /// and appends them to the appropriate registry.
    pub fn record_packet(&mut self, packet: &ParsedPacket) {
        match packet.direction {
            crate::Direction::Incoming => {
                self.bytes_received += packet.total_length as u32;
            }
            crate::Direction::Outgoing => {
                self.bytes_sent += packet.total_length as u32;
            }
        }

        let stream_id = StreamKey::from_packet(packet);
        // Get or create a tracker for this stream and register the packet.
        // The register_packet method will return a burst if one is completed.
        let (burst, direction) = match self
            .streams
            .entry(stream_id)
            .or_insert_with(|| {
                Tracker::<TrackerState>::new(packet.timestamp, packet.transport.get_ip_proto())
            })
            .register_packet(packet)
        {
            Some((burst, direction)) => (burst, direction),
            None => return,
        };

        // Match the direction of the packet and append the burst to the
        // appropriate registry.
        match direction {
            crate::Direction::Incoming => {
                self.received.extend(burst);
            }
            crate::Direction::Outgoing => {
                self.sent.extend(burst);
            }
        }
    }

    /// reset the sent bytes counter and return the value
    pub fn take_sent(&mut self) -> u32 {
        std::mem::take(&mut self.bytes_sent)
    }

    /// reset the received bytes counter and return the value
    pub fn take_received(&mut self) -> u32 {
        std::mem::take(&mut self.bytes_received)
    }

    /// Perform periodic actions:
    /// - Flush any residual bursts from all trackers.
    /// - Prune streams that have been idle longer than the TCP_STREAM_TIMEOUT.
    pub fn periodic(&mut self) {
        for stream in self.streams.values_mut() {
            // Take residual bursts.
            let (sent, received) = match stream.state {
                TrackerState::Tcp(ref mut tracker) => tracker.take_bursts(),
                TrackerState::Udp(ref mut tracker) => tracker.take_bursts(),
                TrackerState::Other(ref mut tracker) => tracker.take_bursts(),
            };
            self.sent.extend(sent);
            self.received.extend(received);
        }
        self.streams.retain(|_, t| {
            // Keep only streams active within the timeout
            t.last_registered.elapsed().unwrap() < crate::Settings::TCP_STREAM_TIMEOUT
        });
    }

    pub fn take_streams(&mut self, keys: Vec<StreamKey>) -> Vec<Tracker<TrackerState>> {
        let mut taken = Vec::new();

        for key in keys {
            if let Some(tracker) = self.streams.remove(&key) {
                taken.push(tracker);
            }
        }
        taken
    }

    pub fn get_streams(&self, protocol: IpNextHeaderProtocol) -> Vec<&Tracker<TrackerState>> {
        self.streams
            .values()
            .filter(|t| t.protocol == protocol)
            .collect()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `default()` initializes all counters and registries to zero or empty.
    #[test]
    fn test_default_initial_state() {
        let mut mgr = StreamManager::default();
        assert_eq!(mgr.take_sent(), 0, "bytes_sent should start at 0");
        assert_eq!(mgr.take_received(), 0, "bytes_received should start at 0");
        assert!(mgr.last_iperf.is_none(), "no iperf timestamp initially");
        assert_eq!(mgr.tcp_thput(), 0.0, "throughput should be zero with no measurements");
        assert!(mgr.sent.pgm_estimator.dps.is_empty(), "sent registry should be empty");
        assert!(mgr.received.pgm_estimator.dps.is_empty(), "received registry should be empty");
    }

    /// Ensure `record_iperf_result` updates `last_iperf` and `tcp_thput`,
    #[test]
    fn test_record_iperf_and_thput_within_window() {
        let mut mgr = StreamManager::default();
        mgr.record_iperf_result(42.5, None);
        assert!(mgr.last_iperf.is_some(), "last_iperf should be set");
        // Immediately after recording, elapsed < window → throughput must be 0.0
        assert_eq!(mgr.tcp_thput(), 0.0, "within window, reported throughput is 0.0");
    }

    /// Verify that `take_sent` and `take_received` reset counters to zero.
    #[test]
    fn test_take_counters_reset() {
        let mut mgr = StreamManager::default();
        // Simulate bytes being counted (use wrapping to avoid overflow panic)
        mgr.bytes_sent = 100;
        mgr.bytes_received = 200;
        assert_eq!(mgr.take_sent(), 100, "should return previous sent bytes");
        assert_eq!(mgr.take_sent(), 0, "counter resets to 0 after take_sent");
        assert_eq!(mgr.take_received(), 200, "should return previous received bytes");
        assert_eq!(mgr.take_received(), 0, "counter resets to 0 after take_received");
    }
}