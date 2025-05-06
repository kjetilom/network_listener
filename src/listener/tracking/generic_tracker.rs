use crate::listener::packet::ParsedPacket;
use crate::{Direction, PacketType};
use pnet::packet::ip::IpNextHeaderProtocol;

use super::tcp_tracker::Burst;

/// Tracks transport-layer packets of a specific protocol, grouping them into bursts.
///
/// A `GenericTracker` accumulates incoming and outgoing `PacketType` instances,
/// resetting and emitting bursts when a sufficient time gap has elapsed or
/// when the burst grows too large.
///
/// This tracker is not currently used for any specific purpose, but serves as a
/// placeholder for future implementations which may require tracking
/// other transport-layer protocols.
#[derive(Debug)]
pub struct GenericTracker {
    /// Protocol type of the packets being tracked.
    pub protocol: IpNextHeaderProtocol,
    /// Incoming burst of packets.
    burst_in: Vec<PacketType>,
    /// Outgoing burst of packets.
    burst_out: Vec<PacketType>,
    /// Timestamp of the last incoming packet.
    last_in: std::time::SystemTime,
    /// Timestamp of the last outgoing packet.
    last_out: std::time::SystemTime,
}

impl GenericTracker {
    /// Creates a new `GenericTracker` for the given IP protocol.
    pub fn new(protocol: IpNextHeaderProtocol) -> Self {
        GenericTracker {
            protocol,
            burst_in: Vec::new(),
            burst_out: Vec::new(),
            last_in: std::time::SystemTime::UNIX_EPOCH,
            last_out: std::time::SystemTime::UNIX_EPOCH,
        }
    }

    /// Registers a parsed packet, returning a completed burst if one has just ended.
    ///
    /// A burst ends and is emitted when the time since the last packet
    /// in its direction exceeds 1 second, or when the burst reaches 100 packets.
    ///
    /// # Arguments
    ///
    /// * `packet`: Parsed packet to register.
    ///
    /// # Returns
    ///
    /// - `Some((Burst, Direction))` if a burst was completed and is returned,
    ///   along with its direction.
    /// - `None` if no burst has completed yet.
    pub fn register_packet(&mut self, packet: &ParsedPacket) -> Option<(Burst, Direction)> {
        let mut ret = Vec::new();

        // Choose the burst and last timestamp based on direction
        let (mut burst, last) = match packet.direction {
            Direction::Incoming => (&mut self.burst_in, &mut self.last_in),
            Direction::Outgoing => (&mut self.burst_out, &mut self.last_out),
        };

        if let Ok(dur) = packet.timestamp.duration_since(*last) {
            if dur > std::time::Duration::from_secs(1) || burst.len() == 100 {
                std::mem::swap(&mut ret, &mut burst);
            }
        }

        // Add this packet and update last timestamp
        burst.push(PacketType::from_packet(packet));
        *last = packet.timestamp;

        if ret.is_empty() {
            None
        } else {
            Some((Burst::Udp(ret), packet.direction))
        }
    }

    /// Takes all accumulated bursts, resetting the tracker.
    ///
    /// # Returns
    /// - A pair `(incoming_burst, outgoing_burst)`, each wrapped in `Burst::Other`.
    pub fn take_bursts(&mut self) -> (Burst, Burst) {
        let mut in_burst = Vec::new();
        let mut out_burst = Vec::new();
        std::mem::swap(&mut in_burst, &mut self.burst_in);
        std::mem::swap(&mut out_burst, &mut self.burst_out);
        (Burst::Other(in_burst), Burst::Other(out_burst))
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use pnet::packet::ip::IpNextHeaderProtocols;

    #[test]
    fn test_new() {
        let tracker = GenericTracker::new(IpNextHeaderProtocols::Udp);
        assert_eq!(tracker.protocol, IpNextHeaderProtocols::Udp);
    }

    #[test]
    fn test_take_bursts_empty() {
        let mut tracker = GenericTracker::new(IpNextHeaderProtocols::Tcp);
        let (in_burst, out_burst) = tracker.take_bursts();
        match in_burst {
            Burst::Other(vec) => assert!(vec.is_empty()),
            _ => panic!("Expected Other variant for incoming burst"),
        }
        match out_burst {
            Burst::Other(vec) => assert!(vec.is_empty()),
            _ => panic!("Expected Other variant for outgoing burst"),
        }
    }
}
