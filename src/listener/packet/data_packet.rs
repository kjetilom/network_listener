// Used to store packets which are acked, or sent (udp) or received (tcp) packets.

use std::ops::{Deref, DerefMut};


/// Represents a data packet with timing and transmission metadata.
///
/// Stores payload length, total packet length, timestamps for when the packet was sent
/// and acknowledged, gaps between successive sends and acknowledgments, retransmission count,
/// and round-trip time (RTT) if available.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct DataPacket {
    /// Length of the payload in bytes.
    pub payload_len: u16,
    /// Total length of the packet in bytes. (headers + payload)
    pub total_length: u16,
    /// Timestamp when the packet was sent.
    pub sent_time: std::time::SystemTime,
    /// Timestamp when the packet was acknowledged.
    pub ack_time: Option<std::time::SystemTime>,
    /// Time gap between the last acknowledgment and the current packet.
    pub gap_last_ack: Option<std::time::Duration>,
    /// Time gap between the last sent packet and the current packet.
    pub gap_last_sent: Option<std::time::Duration>,
    /// Number of retransmissions for this packet.
    pub retransmissions: u8,
    /// Round-trip time (RTT) for this packet, if available.
    pub rtt: Option<tokio::time::Duration>, // TODO: Change to u32 micros duration is 13 bytes
}

/// Classification of a packet as either sent or received.
#[derive(Debug)]
pub enum PacketType {
    /// A packet that was sent from the local host.
    Sent(DataPacket),
    /// A packet that was received by the local host.
    Received(DataPacket),
}

impl PacketType {
    /// Constructs a `PacketType` from a generic parsed packet.
    ///
    /// Determines the direction (incoming or outgoing) and wraps the
    /// corresponding `DataPacket` created from the parsed data.
    pub fn from_packet(packet: &crate::ParsedPacket) -> Self {
        match packet.direction {
            crate::Direction::Incoming => PacketType::Received(DataPacket::from_packet(packet)),
            crate::Direction::Outgoing => PacketType::Sent(DataPacket::from_packet(packet)),
        }
    }

    /// Returns the `Direction` associated with this `PacketType`.
    pub fn direction(&self) -> crate::Direction {
        match self {
            PacketType::Sent(_) => crate::Direction::Outgoing,
            PacketType::Received(_) => crate::Direction::Incoming,
        }
    }
}

impl Deref for PacketType {
    type Target = DataPacket;

    /// Dereferences to the inner `DataPacket` for shared access.
    fn deref(&self) -> &Self::Target {
        match self {
            PacketType::Sent(packet) => packet,
            PacketType::Received(packet) => packet,
        }
    }
}

impl DerefMut for PacketType {
    /// Dereferences to the inner `DataPacket` for mutable access.
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            PacketType::Sent(packet) => packet,
            PacketType::Received(packet) => packet,
        }
    }
}

impl DataPacket {
    /// Creates a new `DataPacket` with the provided parameters.
    ///
    /// # Parameters
    /// - `payload_len`: Length of the packet payload in bytes.
    /// - `total_length`: Total length of the packet (header + payload) in bytes.
    /// - `sent_time`: Timestamp when the packet was sent.
    /// - `ack_time`: Optional timestamp when the packet was acknowledged.
    /// - `gap_last_ack`: Optional duration since the previous acknowledgment.
    /// - `gap_last_sent`: Optional duration since the last sent packet.
    /// - `retransmissions`: Number of retransmissions for this packet.
    /// - `rtt`: Optional measured round-trip time.
    ///
    /// # Returns
    /// Constructed `DataPacket` instance.
    pub fn new(
        payload_len: u16,
        total_length: u16,
        sent_time: std::time::SystemTime,
        ack_time: Option<std::time::SystemTime>,
        gap_last_ack: Option<std::time::Duration>,
        gap_last_sent: Option<std::time::Duration>,
        retransmissions: u8,
        rtt: Option<tokio::time::Duration>,
    ) -> Self {
        DataPacket {
            payload_len,
            total_length,
            sent_time,
            ack_time,
            gap_last_ack,
            gap_last_sent,
            retransmissions,
            rtt,
        }
    }

    /// Returns an "empty" `DataPacket` with zeroed lengths and UNIX epoch timestamp.
    pub fn empty() -> Self {
        DataPacket {
            payload_len: 0,
            total_length: 0,
            sent_time: std::time::SystemTime::UNIX_EPOCH,
            ack_time: None,
            gap_last_ack: None,
            gap_last_sent: None,
            retransmissions: 0,
            rtt: None,
        }
    }

    /// Retrieves the last send and acknowledgment gaps (in seconds) along with the acknowledgment time.
    ///
    /// # Returns
    /// - `Some((gin, gout, ack_time))` if `gap_last_sent`, `gap_last_ack`, and `ack_time` are all `Some`.
    ///   - `gin`: Time gap (s) since the last sent packet.
    ///   - `gout`: Time gap (s) since the last acknowledgment.
    ///   - `ack_time`: Timestamp of the acknowledgment.
    /// - `None` if any of these fields are unavailable.
    pub fn get_gin_gout(&self) -> Option<(f64, f64, std::time::SystemTime)> {
        match (self.gap_last_sent, self.gap_last_ack, self.ack_time) {
            (Some(gin), Some(gout), Some(ack_time)) => Some((
                gin.as_secs_f64(),
                gout.as_secs_f64(),
                ack_time,
            )),
            _ => None,
        }
    }

    /// Constructs a `DataPacket` from a lower-level `ParsedPacket`.
    ///
    /// Extracts the payload length and total length, sets the sent time,
    /// and leaves timing and retransmission metadata unset, for later filling.
    pub fn from_packet(packet: &crate::ParsedPacket) -> Self {
        match packet.transport {
            crate::TransportPacket::TCP { payload_len, .. } => DataPacket {
                payload_len,
                total_length: packet.total_length,
                sent_time: packet.timestamp,
                ack_time: None,
                gap_last_ack: None,
                gap_last_sent: None,
                retransmissions: 0,
                rtt: None,
            },
            crate::TransportPacket::UDP { payload_len, .. } => DataPacket {
                payload_len,
                total_length: packet.total_length,
                sent_time: packet.timestamp,
                ack_time: None,
                gap_last_ack: None,
                gap_last_sent: None,
                retransmissions: 0,
                rtt: None,
            },
            _ => DataPacket {
                payload_len: 0,
                total_length: packet.total_length,
                sent_time: packet.timestamp,
                ack_time: None,
                gap_last_ack: None,
                gap_last_sent: None,
                retransmissions: 0,
                rtt: None,
            },
        }
    }

    pub fn cmp_by_sent_time(&self, b: &DataPacket) -> std::cmp::Ordering {
        self.sent_time.cmp(&b.sent_time)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::Direction;
    use std::time::{SystemTime, Duration as StdDuration};
    use tokio::time::Duration as TokioDuration;
    use std::cmp::Ordering;

    #[test]
    fn test_new_and_empty() {
        let now = SystemTime::now();
        let dp = DataPacket::new(
            10,
            20,
            now,
            Some(now),
            Some(StdDuration::new(1, 0)),
            Some(StdDuration::new(2, 0)),
            3,
            Some(TokioDuration::from_secs(5)),
        );
        assert_eq!(dp.payload_len, 10);
        assert_eq!(dp.total_length, 20);
        assert_eq!(dp.sent_time, now);
        assert_eq!(dp.ack_time, Some(now));
        assert_eq!(dp.gap_last_ack, Some(StdDuration::new(1, 0)));
        assert_eq!(dp.gap_last_sent, Some(StdDuration::new(2, 0)));
        assert_eq!(dp.retransmissions, 3);
        assert_eq!(dp.rtt, Some(TokioDuration::from_secs(5)));

        let empty = DataPacket::empty();
        assert_eq!(empty.payload_len, 0);
        assert_eq!(empty.total_length, 0);
        assert_eq!(empty.sent_time, SystemTime::UNIX_EPOCH);
        assert_eq!(empty.ack_time, None);
        assert_eq!(empty.gap_last_ack, None);
        assert_eq!(empty.gap_last_sent, None);
        assert_eq!(empty.retransmissions, 0);
        assert_eq!(empty.rtt, None);
    }

    #[test]
    fn test_get_gin_gout_some_and_none() {
        let now = SystemTime::now();
        let dp_some = DataPacket::new(
            0,
            0,
            now,
            Some(now),
            Some(StdDuration::new(3, 500_000_000)),
            Some(StdDuration::new(1, 250_000_000)),
            0,
            None,
        );
        let result = dp_some.get_gin_gout();
        assert!(result.is_some());
        let (gin, gout, ack_time) = result.unwrap();
        assert!((gin - 1.25).abs() < f64::EPSILON);
        assert!((gout - 3.5).abs() < f64::EPSILON);
        assert_eq!(ack_time, now);

        let dp_none = DataPacket::empty();
        assert!(dp_none.get_gin_gout().is_none());
    }

    #[test]
    fn test_cmp_by_sent_time() {
        let t1 = SystemTime::UNIX_EPOCH + StdDuration::new(100, 0);
        let t2 = SystemTime::UNIX_EPOCH + StdDuration::new(200, 0);
        let dp1 = DataPacket::new(0, 0, t1, None, None, None, 0, None);
        let dp2 = DataPacket::new(0, 0, t2, None, None, None, 0, None);
        assert_eq!(dp1.cmp_by_sent_time(&dp2), Ordering::Less);
        assert_eq!(dp2.cmp_by_sent_time(&dp1), Ordering::Greater);
        assert_eq!(dp1.cmp_by_sent_time(&dp1), Ordering::Equal);
    }

    #[test]
    fn test_packet_type_direction_and_deref() {
        let dp = DataPacket::empty();
        let pt_sent = PacketType::Sent(dp);
        assert_eq!(pt_sent.direction(), Direction::Outgoing);
        assert_eq!(pt_sent.payload_len, 0);

        let pt_recv = PacketType::Received(dp);
        assert_eq!(pt_recv.direction(), Direction::Incoming);
        assert_eq!(pt_recv.total_length, 0);
    }
}
