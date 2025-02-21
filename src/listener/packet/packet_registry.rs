use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut}, time::{Duration, SystemTime},
};
use num_traits::Float;
use yata::methods::EMA;
use yata::prelude::*;
use super::DataPacket;

#[derive(Debug)]
pub struct PacketRegistry {
    packets: VecDeque<DataPacket>,
    // Probe gap model dps (gout/gack, burst_avg_packet_size/gack)
    pgm_data: VecDeque<(f64, f64)>,
    sum_rtt: (f64, u16),
    min_rtt: f64,
    last_ema: EMA,
    sum_data: u32,
    retransmissions: u16,
}

impl PacketRegistry {
    pub fn new(size: usize) -> Self {
        PacketRegistry {
            packets: VecDeque::with_capacity(size),
            pgm_data: VecDeque::new(),
            sum_rtt: (0.0, 0),
            min_rtt: f64::MAX,
            last_ema: EMA::new(20, &0.0).unwrap(),
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

    pub fn get_rtts(&mut self, normalize_rtts: bool) -> Vec<DataPacket> {
        let ret: Vec<DataPacket> = self.packets
            .drain(..)
            .filter_map(|packet| {
                if packet.rtt.is_some() {
                    Some(packet)
                } else {
                    None
                }
            }).collect();

        if normalize_rtts {
            let bursts = self.bursts(ret.into_iter());
            self.normalize_rtt_bursts(bursts).into_iter().flatten().collect()
        } else {
            ret
        }
    }

    pub fn passive_pgm_abw(&self) -> Option<f64> {
        if self.pgm_data.is_empty() {
            return None;
        }

        let n = self.pgm_data.len() as f64;
        // gout / gack
        let y: Vec<f64> = self.pgm_data.iter().map(|(gout_gack, _)| *gout_gack).collect();
        // Packet size / gack
        let x: Vec<f64> = self.pgm_data.iter().map(|(_, lgack)| *lgack).collect();

        // Compute sums needed for least squares regression.
        let sum_x: f64 = x.iter().sum();
        let sum_y: f64 = y.iter().sum();
        let sum_xy: f64 = x.iter().zip(y.iter()).map(|(xi, yi)| xi * yi).sum();
        let sum_x2: f64 = x.iter().map(|xi| xi * xi).sum();

        // Compute the slope and intercept of the line of best fit.
        let numerator = n * sum_xy - sum_x * sum_y;
        let denom = n * sum_x2 - sum_x * sum_x;
        let a = numerator / denom;
        let b = (sum_y - a * sum_x) / n;

        if a != 0.0 {
            return Some((1.0 - b) / a)
        }
        None
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

            self.min_rtt = self.min_rtt.min(rtt.as_secs_f64());
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

    pub fn bursts(&self, mut packet_iter: impl Iterator<Item = DataPacket>) -> Vec<Vec<DataPacket>> {
        let mut bursts = Vec::new();

        let mut current_burst = match packet_iter.next() {
            Some(packet) => vec![packet],
            None => return bursts,
        };

        for packet in packet_iter {
            let prev_packet = current_burst.last().unwrap();
            let sent_diff = match packet.sent_time.duration_since(prev_packet.sent_time) {
                Ok(sent_diff) => sent_diff,
                Err(_) => continue,
            };

            if sent_diff.as_secs_f64() > self.min_rtt {
                bursts.push(current_burst);
                current_burst = vec![packet];
            } else {
                current_burst.push(packet);
            }
        }
        bursts.push(current_burst);
        bursts
    }

    fn get_abw_params(burst: &Vec<DataPacket>) -> (f64, f64, f64) {
        let mut min_rtt = f64::MAX;
        let mut max_rtt = 0.0;
        let mut sum_sizes = 0.0;
        for packet in burst {
            let rtt = packet.rtt.unwrap().as_secs_f64();
            min_rtt = min_rtt.min(rtt);
            max_rtt = max_rtt.max(rtt);
            sum_sizes += packet.total_length as f64;
        }
        (min_rtt, max_rtt, sum_sizes / burst.len() as f64)
    }

    fn gout_gack(last: &DataPacket, first: &DataPacket) -> (f64, f64) {
        let gout = match last.sent_time.duration_since(first.sent_time) {
            Ok(gout) => gout.as_secs_f64(),
            Err(_) => 0.0,
        };
        let gack = match (last.sent_time + last.rtt.unwrap()).duration_since(first.sent_time + first.rtt.unwrap()) {
            Ok(gack) => gack.as_secs_f64(),
            Err(_) => 0.0,
        };
        (gout, gack)
    }

    fn normalize_rtt_bursts(&mut self, bursts: Vec<Vec<DataPacket>>) -> Vec<Vec<DataPacket>> {
        let mut normalized_bursts = Vec::new();
        for burst in bursts {
            // Not useful data.
            if burst.len() <= 5 {
                continue;
            }

            let (min_rtt, max_rtt, avg_pkt_size) = Self::get_abw_params(&burst);

            let (gout, gack) = Self::gout_gack(burst.last().unwrap(), burst.first().unwrap());
            if gout == 0.0 || gack == 0.0 {
                continue;
            }

            if gout / gack < 5.0 {
                if self.pgm_data.len() == 150 {
                    self.pgm_data.pop_front();
                }
                self.pgm_data.push_back((gout / gack, avg_pkt_size / gack));
            }

            let increase = (max_rtt - min_rtt) / (burst.len() as f64 - 1.0);
            let mut prev_rtt = min_rtt - increase; // Start at min - increase to start at min
            let normalized_rtts: Vec<DataPacket> = burst.iter().map(|packet| {
                prev_rtt = prev_rtt + increase;
                DataPacket {
                    rtt: Some(tokio::time::Duration::from_secs_f64(prev_rtt)),
                    ..*packet
                }
            }).collect();
            normalized_bursts.push(normalized_rtts);
        }
        normalized_bursts
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

    fn create_packets(packet_size: u16, rtt: Duration, rtt_increase: Duration, count: u8) -> Vec<DataPacket> {
        let mut packets = Vec::new();
        let mut sent_time = std::time::SystemTime::now();
        let mut rtt = rtt;
        for _ in 0..count {
            packets.push(DataPacket::new(packet_size, packet_size, sent_time, 0, Some(rtt)));
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
        assert_eq!(registry.mean_rtt(), Some(3.0));
        assert_eq!(registry.avg_pkt_size(), 100.0);
        assert_eq!(registry.retransmissions(), 0);

        let packets = create_packets(100, Duration::from_secs(1), Duration::from_secs(1), 3);
        registry.extend(packets.clone());
        assert_eq!(registry.len(), 5);
        assert_eq!(registry.capacity(), 5);
        assert_eq!(registry.mean_rtt(), Some(3.0));
        assert_eq!(registry.avg_pkt_size(), 100.0);
        assert_eq!(registry.retransmissions(), 0);

        let packets = create_packets(150, Duration::from_secs(1), Duration::from_secs(0), 2);
        registry.extend(packets.clone());
        assert_eq!(registry.len(), 5);
        assert_eq!(registry.capacity(), 5);
        assert_eq!(registry.mean_rtt(), Some(1.6));
        assert_eq!(registry.avg_pkt_size(), 120.0);
        assert_eq!(registry.retransmissions(), 0);

        let packets = create_packets(200, Duration::from_secs(1), Duration::from_secs(0), 1);
        registry.extend(packets.clone());
        assert_eq!(registry.len(), 5);
        assert_eq!(registry.capacity(), 5);
        assert_eq!(registry.mean_rtt(), Some(1.6));
        assert_eq!(registry.avg_pkt_size(), 140.0);
        assert_eq!(registry.retransmissions(), 0);
    }

    #[test]
    fn test_get_rtts_ema() {
        let mut registry = PacketRegistry::new(5);
        let packets = create_packets(100, Duration::from_secs(1), Duration::from_secs(1), 5);
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

    #[test]
    fn test_drain_rtts_all_some_packet_with_rtt() {
        let mut registry = PacketRegistry::new(5);
        let packets = vec![
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, Some(tokio::time::Duration::from_secs(1))),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, Some(tokio::time::Duration::from_secs(1))),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, Some(tokio::time::Duration::from_secs(2))),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, Some(tokio::time::Duration::from_secs(2))),
            DataPacket::new(100, 100, std::time::SystemTime::now(), 0, Some(tokio::time::Duration::from_secs(1))),
        ];
        registry.extend(packets.clone());
        assert_eq!(registry.len(), 5);
        assert_eq!(registry.capacity(), 5);
        assert_eq!(registry.mean_rtt(), Some(1.4));
        assert_eq!(registry.min_rtt, 1.0);
        assert_eq!(registry.avg_pkt_size(), 100.0);
        assert_eq!(registry.retransmissions(), 0);

        let rtts = registry.get_rtts(true);
        assert_eq!(rtts.len(), 5);
        assert_eq!(rtts[0].rtt, Some(tokio::time::Duration::from_secs(1)));
        assert_eq!(rtts[1].rtt, Some(tokio::time::Duration::from_secs_f64(1.25)));
        assert_eq!(rtts[2].rtt, Some(tokio::time::Duration::from_secs_f64(1.5)));
        assert_eq!(rtts[3].rtt, Some(tokio::time::Duration::from_secs_f64(1.75)));
        assert_eq!(rtts[4].rtt, Some(tokio::time::Duration::from_secs(2)));
    }

    #[test]
    fn test_burst_params() {
        let mut packets = create_packets(100, Duration::from_secs(1), Duration::from_secs(1), 20);

        let (min_rtt, max_rtt, avg_pkt_size) = PacketRegistry::get_abw_params(&packets);
        assert_eq!(min_rtt, 1.0);
        assert_eq!(max_rtt, 20.0);
        assert_eq!(avg_pkt_size, 100.0);
        packets.extend(create_packets(200, Duration::from_secs(1), Duration::from_secs(1), 20));
        let (min_rtt, max_rtt, avg_pkt_size) = PacketRegistry::get_abw_params(&packets);
        assert_eq!(min_rtt, 1.0);
        assert_eq!(max_rtt, 20.0);
        assert_eq!(avg_pkt_size, 150.0);
    }

    #[test]
    fn test_registry_is_sorted() {
        let mut registry = PacketRegistry::new(200);
        let time = std::time::SystemTime::now();
        for i in 0..100 {
            let dur = std::time::Duration::from_millis(i*2);
            let packet = DataPacket::new(100, 100, time+dur, 0, Some(tokio::time::Duration::from_secs(1)));
            registry.push(packet);
        }

        let old_packets = vec![
            DataPacket::new(100, 100, time, 0, Some(tokio::time::Duration::from_secs(1))),
            DataPacket::new(100, 100, time+std::time::Duration::from_millis(2), 0, Some(tokio::time::Duration::from_secs(1))),
            DataPacket::new(100, 100, time+std::time::Duration::from_millis(4), 0, Some(tokio::time::Duration::from_secs(1))),
            DataPacket::new(100, 100, time+std::time::Duration::from_millis(6), 0, Some(tokio::time::Duration::from_secs(1))),
            DataPacket::new(100, 100, time+std::time::Duration::from_millis(8), 0, Some(tokio::time::Duration::from_secs(1))),
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
