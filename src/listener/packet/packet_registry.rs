use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut}, time::Duration
};
use super::DataPacket;
use super::estimation::PABWESender;

#[derive(Debug)]
pub struct PacketRegistry {
    packets: VecDeque<DataPacket>,
    pgm_estimator: PABWESender,
    min_rtt: f64,
    sum_data: u32,
    retransmissions: u16,
}

impl PacketRegistry {
    pub fn new(size: usize) -> Self {
        PacketRegistry {
            packets: VecDeque::with_capacity(size),
            pgm_estimator: PABWESender::new(Some(Duration::from_secs(30))),
            min_rtt: f64::MAX,
            sum_data: 0,
            retransmissions: 0,
        }
    }

    pub fn iter_packets_rtt(&self) -> impl Iterator<Item = &DataPacket> {
        self.packets.iter().filter(|p| p.rtt.is_some())
    }

    pub fn iter_packets_rtt_mut(&mut self) -> impl Iterator<Item = &mut DataPacket> {
        self.packets.iter_mut().filter(|p| p.rtt.is_some())
    }

    fn add_values(&mut self, packet: &DataPacket) {
        if let Some(rtt) = packet.rtt {
            self.min_rtt = self.min_rtt.min(rtt.as_secs_f64());
        }
        self.sum_data += packet.total_length as u32;
        self.retransmissions += packet.retransmissions as u16;
    }

    fn sub_values(&mut self, packet: &DataPacket) {
        self.sum_data -= packet.total_length as u32;
        self.retransmissions -= packet.retransmissions as u16;
    }

    pub fn passive_pgm_abw(&mut self) -> Option<f64> {
        self.pgm_estimator.passive_pgm_abw()
    }

    pub fn get_rtts(&mut self) -> Vec<DataPacket> {
        let rtts: Vec<DataPacket> = self.packets.drain(..).filter(|p| p.rtt.is_some()).collect();
        self.pgm_estimator.iter_packets(&rtts);
        rtts
    }

    pub fn push(&mut self, value: DataPacket) {
        self.add_values(&value);

        if self.len() == self.capacity() {
            let old = self.pop_front().unwrap();
            self.sub_values(&old);
        }

        let mut insert_idx = self.len();
        // Iter backwards to find the correct index to insert the packet.
        // Packets should be sorted by sent_time in increasing order.
        for packet in self.iter().rev() {
            if packet.sent_time <= value.sent_time {
                break;
            }
            insert_idx -= 1;
        }
        self.insert(insert_idx, value);
    }

    pub fn extend(&mut self, values: Vec<DataPacket>) {
        for value in values {
            self.push(value);
        }
    }

    pub fn avg_rtt(&self) -> Option<f64> {
        let rtts: Vec<f64> = self.iter_packets_rtt().map(|p| p.rtt.unwrap().as_secs_f64()).collect();
        if rtts.is_empty() {
            None
        } else {
            Some(rtts.iter().sum::<f64>() / rtts.len() as f64)
        }
    }

    pub fn min_rtt(&self) -> Option<f64> {
        if self.min_rtt == f64::MAX {
            None
        } else {
            Some(self.min_rtt)
        }
    }

    pub fn retransmissions(&self) -> u16 {
        self.retransmissions
    }

    pub fn avg_burst_thp(&self) -> Option<f64> {
        let thpts = PABWESender::get_burst_thp(self.iter_packets_rtt().cloned().collect(), Duration::from_secs_f64(self.min_rtt));
        if thpts.is_empty() {
            None
        } else {
            Some(thpts.iter().sum::<f64>() / thpts.len() as f64)
        }
    }

    pub fn loss(&self) -> f64 {
        if self.retransmissions == 0 {
            0.0
        } else {
            self.retransmissions as f64 / self.len() as f64
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_packets(packet_size: u16, rtt: Duration, rtt_increase: Duration, count: u8) -> Vec<DataPacket> {
        let mut packets = Vec::new();
        let mut sent_time = std::time::SystemTime::now();
        let mut rtt = rtt;
        for _ in 0..count {
            packets.push(DataPacket::new(packet_size, packet_size, sent_time, Some(sent_time + rtt), 0, Some(rtt)));
            rtt = rtt + rtt_increase;
            sent_time += Duration::from_millis(1);
        }
        packets
    }

    #[test]
    fn test_packet_registry() {
        let mut registry = PacketRegistry::new(5);

        let packets = create_packets(100, Duration::from_secs(1), Duration::from_secs(1), 5);
        registry.extend(packets.clone());
        assert_eq!(registry.len(), 5);
        assert_eq!(registry.capacity(), 5);
        assert_eq!(registry.retransmissions(), 0);

        let packets = create_packets(100, Duration::from_secs(1), Duration::from_secs(1), 3);
        registry.extend(packets.clone());
        assert_eq!(registry.len(), 5);
        assert_eq!(registry.capacity(), 5);
        assert_eq!(registry.retransmissions(), 0);

        let packets = create_packets(150, Duration::from_secs(1), Duration::from_secs(0), 2);
        registry.extend(packets.clone());
        assert_eq!(registry.len(), 5);
        assert_eq!(registry.capacity(), 5);
        assert_eq!(registry.retransmissions(), 0);

        let packets = create_packets(200, Duration::from_secs(1), Duration::from_secs(0), 1);
        registry.extend(packets.clone());
        assert_eq!(registry.len(), 5);
        assert_eq!(registry.capacity(), 5);
        assert_eq!(registry.retransmissions(), 0);
    }

    #[test]
    fn test_registry_is_sorted() {
        let mut registry = PacketRegistry::new(200);
        let time = std::time::SystemTime::now();
        for i in 0..100 {
            let dur = std::time::Duration::from_millis(i*2);
            let packet = DataPacket::new(100, 100, time+dur, None, 0, Some(tokio::time::Duration::from_secs(1)));
            registry.push(packet);
        }

        let old_packets = vec![
            DataPacket::new(100, 100, time, None, 0, Some(tokio::time::Duration::from_secs(1))),
            DataPacket::new(100, 100, time+std::time::Duration::from_millis(2), None, 0, Some(tokio::time::Duration::from_secs(1))),
            DataPacket::new(100, 100, time+std::time::Duration::from_millis(1), None, 0, Some(tokio::time::Duration::from_secs(1))),
            DataPacket::new(100, 100, time+std::time::Duration::from_millis(6), None, 0, Some(tokio::time::Duration::from_secs(1))),
            DataPacket::new(100, 100, time+std::time::Duration::from_millis(7), None, 0, Some(tokio::time::Duration::from_secs(1))),
        ];
        registry.extend(old_packets.clone());
        // Check if registry is sorted
        let mut prev_time = time;
        for packet in registry.iter() {
            assert!(packet.sent_time >= prev_time);
            prev_time = packet.sent_time;
        }
    }
}
