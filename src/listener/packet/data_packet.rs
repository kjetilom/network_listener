// Used to store packets which are acked, or sent (udp) or received (tcp) packets.

use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut}, time::{Duration, SystemTime},
};
use yata::methods::EMA;
use yata::prelude::*;

#[derive(Debug)]
pub struct PacketRegistry {
    packets: VecDeque<DataPacket>,
    sum_rtt: (f64, u16),
    last_ema: EMA,
    sum_data: u32,
    retransmissions: u16,
    max_delivery_rate_samples: VecDeque<f64>,
    max_delivery_rate: f64,
}

impl PacketRegistry {
    pub fn new(size: usize) -> Self {
        PacketRegistry {
            packets: VecDeque::with_capacity(size),
            sum_rtt: (0.0, 0),
            last_ema: EMA::new(20, &0.0).unwrap(),
            sum_data: 0,
            retransmissions: 0,
            max_delivery_rate_samples: VecDeque::new(),
            max_delivery_rate: 0.0,
        }
    }

    pub fn get_rtts(&mut self) -> Vec<DataPacket> {
        self.packets
            .drain(..)
            .filter_map(|packet| {
                if packet.rtt.is_some() {
                    Some(packet)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn get_rtts_ema(&mut self) -> Vec<DataPacket> {
        if self.is_empty() {
            return Vec::new();
        }

        let mut ema = self.last_ema;
        let ret = self.packets
            .drain(..)
            .filter_map(|packet| {
                if packet.rtt.is_some() {
                    Some(DataPacket {
                        rtt: Some(tokio::time::Duration::from_secs_f64(
                            ema.next(&packet.rtt.unwrap().as_secs_f64()),
                        )),
                        ..packet
                    })
                } else {
                    None
                }
            })
            .collect();
        self.last_ema = ema;
        ret
    }

    fn add_values(&mut self, packet: &DataPacket) {
        if let Some(rtt) = packet.rtt {
            self.sum_rtt.0 += rtt.as_secs_f64();
            self.sum_rtt.1 += 1;
        }
        self.sum_data += packet.total_length as u32;
        self.retransmissions += packet.retransmissions as u16;
    }

    fn sub_values(&mut self, packet: &DataPacket) {
        if let Some(rtt) = packet.rtt {
            self.sum_rtt.0 -= rtt.as_secs_f64();
            self.sum_rtt.1 -= 1;
        }
        self.sum_data -= packet.total_length as u32;
        self.retransmissions -= packet.retransmissions as u16;
    }

    pub fn push(&mut self, value: DataPacket) {
        self.add_values(&value);

        if self.len() == self.capacity() {
            let old = self.pop_front().unwrap();
            self.sub_values(&old);
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
            if self.sum_rtt.1 == 0 {
                return None;
            }
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

    pub fn estimate_available_bandwidth(&self, time_window: Duration) -> Option<f64> {
        if self.is_empty() {
            return None;
        }

        let now = SystemTime::now();
        let mut total_bytes = 0;

        // Iterate in reverse order (most recent packets first)
        for packet in self.packets.iter().rev() {
            let packet_age = now.duration_since(packet.sent_time).ok()?;

            if packet_age > time_window {
                break; // Stop when we reach the beginning of the time window
            }

            total_bytes += packet.total_length as u64;
        }

        if total_bytes == 0 {
            return None;
        }

        // Calculate throughput in bits per second
        let time_window_seconds = time_window.as_secs_f64();
        let bits_per_second = (total_bytes as f64 * 8.0) / time_window_seconds;

        Some(bits_per_second)
    }

    const MAX_SAMPLES: usize = 10;

    pub fn update_max_delivery_rate(&mut self, delivery_rate: f64) {
        self.max_delivery_rate_samples.push_back(delivery_rate);
        if self.max_delivery_rate_samples.len() > Self::MAX_SAMPLES {
            self.max_delivery_rate_samples.pop_front();
        }

        // Update max_delivery_rate to the maximum value in the samples
        self.max_delivery_rate = self
            .max_delivery_rate_samples
            .iter()
            .cloned()
            .fold(0.0, f64::max);
    }

    pub fn estimate_available_bandwidth_bbr(&mut self) -> Option<f64> {
        if self.is_empty() {
            return None;
        }

        // Calculate delivery rate for each packet with RTT
        for packet in self.packets.iter() {
            if let Some(rtt) = packet.rtt {
                let rtt_seconds = rtt.as_secs_f64();
                let delivery_rate = packet.total_length as f64 * 8.0 / rtt_seconds; // bits per second
                self.max_delivery_rate_samples.push_back(delivery_rate);
                if self.max_delivery_rate_samples.len() > Self::MAX_SAMPLES {
                    self.max_delivery_rate_samples.pop_front();
                }

                // Update max_delivery_rate to the maximum value in the samples
                self.max_delivery_rate = self
                    .max_delivery_rate_samples
                    .iter()
                    .cloned()
                    .fold(0.0, f64::max);
                }
        }

        Some(self.max_delivery_rate)
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
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_registry() {
        let mut registry = PacketRegistry::new(5);
        let packets = vec![
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, None),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, None),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, None),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, None),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, None),
        ];
        registry.extend(packets.clone());
        assert_eq!(registry.len(), 5);
        assert_eq!(registry.capacity(), 5);
        assert_eq!(registry.mean_rtt(), None);
        assert_eq!(registry.avg_pkt_size(), 100.0);
        assert_eq!(registry.retransmissions(), 0);

        let packets = vec![
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, None),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, None),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, None),
        ];
        registry.extend(packets.clone());
        assert_eq!(registry.len(), 5);
        assert_eq!(registry.capacity(), 5);
        assert_eq!(registry.mean_rtt(), None);
        assert_eq!(registry.avg_pkt_size(), 100.0);
        assert_eq!(registry.retransmissions(), 0);

        let packets = vec![
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, None),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, None),
        ];
        registry.extend(packets.clone());
        assert_eq!(registry.len(), 5);
        assert_eq!(registry.capacity(), 5);
        assert_eq!(registry.mean_rtt(), None);
        assert_eq!(registry.avg_pkt_size(), 100.0);
        assert_eq!(registry.retransmissions(), 0);

        let packets = vec![DataPacket::new(
            100,
            100,
            std::time::SystemTime::now(),
            0,
            Some(tokio::time::Duration::from_secs(1)),
        )];
        registry.extend(packets.clone());
        assert_eq!(registry.len(), 5);
        assert_eq!(registry.capacity(), 5);
        assert_eq!(registry.mean_rtt(), Some(1.0));
        assert_eq!(registry.avg_pkt_size(), 100.0);
        assert_eq!(registry.retransmissions(), 0);
    }

    #[test]
    fn test_get_rtts_ema() {
        let mut registry = PacketRegistry::new(5);
        let packets = vec![
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, Some(tokio::time::Duration::from_secs(1))),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, Some(tokio::time::Duration::from_secs(2))),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, Some(tokio::time::Duration::from_secs(3))),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, Some(tokio::time::Duration::from_secs(4))),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, Some(tokio::time::Duration::from_secs(5))),
        ];
        registry.extend(packets.clone());
        assert_eq!(registry.len(), 5);
        assert_eq!(registry.capacity(), 5);
        assert_eq!(registry.mean_rtt(), Some(3.0));
        assert_eq!(registry.avg_pkt_size(), 100.0);
        assert_eq!(registry.retransmissions(), 0);

        let rtts = registry.get_rtts_ema();
        assert_eq!(rtts.len(), 5);
        assert_eq!(rtts[0].rtt, Some(tokio::time::Duration::from_secs_f64(0.095238095)));

        let ema = registry.last_ema;
        assert_eq!(ema.peek(), 1.2596373106345802);

    }
}
