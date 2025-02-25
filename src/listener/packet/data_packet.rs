// Used to store packets which are acked, or sent (udp) or received (tcp) packets.

use std::ops::{Deref, DerefMut};

/// Single struct to represent a sent or received packet.
/// Should be as small as possible to reduce memory usage.
///
/// # Fields
///
/// * `payload_len` - Length of the packet payload.
/// * `total_length` - Total length of the packet.
/// * `sent_time` - Time when the packet was sent.
/// * `retransmissions` - Number of retransmissions for the packet.
/// * `rtt` - Round trip time to acknowledge the segment.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct DataPacket {
    pub payload_len: u16,
    pub total_length: u16,
    pub sent_time: std::time::SystemTime, // TODO: Change to relative time (system time is ~16 bytes)
    pub ack_time: Option<std::time::SystemTime>,
    pub retransmissions: u8,
    pub rtt: Option<tokio::time::Duration>, // TODO: Change to u32 micros duration is 13 bytes
}

#[derive(Debug)]
pub enum PacketType {
    Sent(DataPacket),
    Received(DataPacket),
}

impl PacketType {
    pub fn from_packet(packet: &crate::ParsedPacket) -> Self {
        match packet.direction {
            crate::Direction::Incoming => PacketType::Received(DataPacket::from_packet(packet)),
            crate::Direction::Outgoing => PacketType::Sent(DataPacket::from_packet(packet)),
        }
    }

    pub fn direction(&self) -> crate::Direction {
        match self {
            PacketType::Sent(_) => crate::Direction::Outgoing,
            PacketType::Received(_) => crate::Direction::Incoming,
        }
    }
}

impl Deref for PacketType {
    type Target = DataPacket;

    fn deref(&self) -> &Self::Target {
        match self {
            PacketType::Sent(packet) => packet,
            PacketType::Received(packet) => packet,
        }
    }
}

impl DerefMut for PacketType {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            PacketType::Sent(packet) => packet,
            PacketType::Received(packet) => packet,
        }
    }
}

impl DataPacket {
    pub fn new(
        payload_len: u16,
        total_length: u16,
        sent_time: std::time::SystemTime,
        ack_time: Option<std::time::SystemTime>,
        retransmissions: u8,
        rtt: Option<tokio::time::Duration>,
    ) -> Self {
        DataPacket {
            payload_len,
            total_length,
            sent_time,
            ack_time,
            retransmissions,
            rtt,
        }
    }

    pub fn to_proto_rtt(self) -> crate::proto_bw::Rtt {
        crate::proto_bw::Rtt {
            rtt: self.rtt.map(|rtt| rtt.as_secs_f64()).unwrap_or(0.0),
            timestamp: self
                .sent_time
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
                .try_into()
                .unwrap(),
        }
    }

    pub fn from_packet(packet: &crate::ParsedPacket) -> Self {
        match packet.transport {
            crate::TransportPacket::TCP { payload_len, .. } => DataPacket {
                payload_len,
                total_length: packet.total_length,
                sent_time: packet.timestamp,
                ack_time: None,
                retransmissions: 0,
                rtt: None,
            },
            crate::TransportPacket::UDP { payload_len, .. } => DataPacket {
                payload_len,
                total_length: packet.total_length,
                sent_time: packet.timestamp,
                ack_time: None,
                retransmissions: 0,
                rtt: None,
            },
            _ => DataPacket {
                payload_len: 0,
                total_length: packet.total_length,
                sent_time: packet.timestamp,
                ack_time: None,
                retransmissions: 0,
                rtt: None,
            },
        }
    }

    pub fn cmp_by_sent_time(&self, b: &DataPacket) -> std::cmp::Ordering {
        self.sent_time.cmp(&b.sent_time)
    }
}


pub struct PgmDataPacket {
    pub total_length: u16,
    pub sent_time: std::time::SystemTime,
    pub ack_time: std::time::SystemTime,
}