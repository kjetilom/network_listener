// Used to store packets which are acked, or sent (udp) or received (tcp) packets.

use std::{
    collections::VecDeque, ops::{Deref, DerefMut}
};

#[derive(Debug)]
pub struct PacketRegistry {
    packets: VecDeque<DataPacket>,
    sum_rtt: (f64, u16),
    sum_data: u32,
    retransmissions: u16,
}

impl PacketRegistry {
    pub fn new(size: usize) -> Self {
        PacketRegistry {
            packets: VecDeque::with_capacity(size),
            sum_rtt: (0.0, 0),
            sum_data: 0,
            retransmissions: 0,
        }
    }

    pub fn push(&mut self, value: DataPacket) {
        self.sum_rtt.0 += value.rtt.unwrap_or_default().as_secs_f64();
        self.sum_rtt.1 += 1;
        self.sum_data += value.total_length as u32;
        self.retransmissions += value.retransmissions as u16;

        if self.len() == self.capacity() {
            let old = self.pop_front().unwrap();
            self.sum_rtt.0 -= old.rtt.unwrap_or_default().as_secs_f64();
            self.sum_rtt.1 -= 1;
            self.sum_data -= old.total_length as u32;
            self.retransmissions -= old.retransmissions as u16;
        }
        self.push_back(value);
    }

    pub fn extend(&mut self, values: Vec<DataPacket>) {
        for value in values {
            self.push(value);
        }
    }

    pub fn mean_rtt(&self) -> Option<f64> {
        if self.is_empty() {
            None
        } else {
            Some(self.sum_rtt.0 / self.sum_rtt.1 as f64)
        }
    }

    pub fn avg_pkt_size(&self) -> f64 {
        if self.is_empty() {
            0.0
        } else {
            self.sum_data as f64 / self.len() as f64
        }
    }

    pub fn retransmissions(&self) -> u16 {
        self.retransmissions
    }

    pub fn clear(&mut self) {
        self.packets.clear();
        self.sum_rtt = (0.0, 0);
        self.sum_data = 0;
    }
}

impl Deref for PacketRegistry {
    type Target = VecDeque<DataPacket>;

    fn deref(&self) -> &Self::Target {
        &self.packets
    }
}

impl DerefMut for PacketRegistry {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.packets
    }
}

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
    pub retransmissions: u8,
    pub rtt: Option<tokio::time::Duration>, // TODO: Change to u32 micros duration is 13 bytes
}

impl DataPacket {
    pub fn new(
        payload_len: u16,
        total_length: u16,
        sent_time: std::time::SystemTime,
        retransmissions: u8,
        rtt: Option<tokio::time::Duration>,
    ) -> Self {
        DataPacket {
            payload_len,
            total_length,
            sent_time,
            retransmissions,
            rtt,
        }
    }

    pub fn from_packet(packet: &crate::ParsedPacket) -> Self {
        match packet.transport {
            crate::TransportPacket::TCP { payload_len, .. } => DataPacket {
                payload_len,
                total_length: packet.total_length,
                sent_time: packet.timestamp,
                retransmissions: 0,
                rtt: None,
            },
            crate::TransportPacket::UDP { payload_len, .. } => DataPacket {
                payload_len,
                total_length: packet.total_length,
                sent_time: packet.timestamp,
                retransmissions: 0,
                rtt: None,
            },
            _ => DataPacket {
                payload_len: 0,
                total_length: packet.total_length,
                sent_time: packet.timestamp,
                retransmissions: 0,
                rtt: None,
            }
        }
    }
}
