use super::estimation::PABWESender;
use super::DataPacket;
use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
    time::{Duration, SystemTime},
};

#[derive(Debug)]
pub struct PacketRegistry {
    packets: VecDeque<DataPacket>,
    pgm_estimator: PABWESender,
    min_rtt: (f64, SystemTime),
    sum_data: u32,
    retransmissions: u16,
}

impl PacketRegistry {
    pub fn new(size: usize) -> Self {
        PacketRegistry {
            packets: VecDeque::with_capacity(size),
            pgm_estimator: PABWESender::new(Some(Duration::from_secs(60))),
            min_rtt: (f64::MAX, SystemTime::now()),
            sum_data: 0,
            retransmissions: 0,
        }
    }

    pub fn min_rtt(&self) -> Option<f64> {
        if self.min_rtt.0 == f64::MAX {
            None
        } else {
            Some(self.min_rtt.0)
        }
    }

    fn reset_min_rtt(&mut self) {
        let min_rtt = self
            .packets
            .iter()
            .filter(|p| p.rtt.is_some())
            .min_by(|a, b| a.rtt.unwrap().cmp(&b.rtt.unwrap()));

        if let Some(min_rtt) = min_rtt {
            self.min_rtt = (min_rtt.rtt.unwrap().as_secs_f64(), min_rtt.ack_time.unwrap());
        } else {
            self.min_rtt = (f64::MAX, SystemTime::now());
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
            let rtt = rtt.as_secs_f64();
            if self.min_rtt.0 > rtt {
                self.min_rtt.0 = rtt;
                self.min_rtt.1 = packet.ack_time.unwrap();
            }
            match packet.ack_time.unwrap().duration_since(self.min_rtt.1) {
                Ok(d) => {
                    if d.as_secs_f64() > self.min_rtt.0 {
                        self.reset_min_rtt();
                    }
                }
                Err(_) => {}
            }
        }
        self.sum_data += packet.total_length as u32;
        self.retransmissions += packet.retransmissions as u16;
    }

    fn sub_values(&mut self, packet: &DataPacket) {
        self.sum_data -= packet.total_length as u32;
        self.retransmissions -= packet.retransmissions as u16;
    }

    pub fn passive_pgm_abw(&mut self) -> Option<f64> {
        if let Some(res) = self.pgm_estimator.passive_pgm_abw() {
            let dps = self.pgm_estimator.drain();
            return Some(res)
        }
        None
    }

    pub fn get_rtts(&mut self) -> Vec<DataPacket> {
        let rtts: Vec<DataPacket> = self
            .packets
            .drain(..)
            .filter(|p| p.gap_last_ack.is_some() && p.gap_last_sent.is_some())
            .collect();
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
        let rtts: Vec<f64> = self
            .iter_packets_rtt()
            .map(|p| p.rtt.unwrap().as_secs_f64())
            .collect();
        if rtts.is_empty() {
            None
        } else {
            Some(rtts.iter().sum::<f64>() / rtts.len() as f64)
        }
    }

    pub fn retransmissions(&self) -> u16 {
        self.retransmissions
    }

    pub fn avg_burst_thp(&self) -> Option<f64> {
        let thpts = PABWESender::get_burst_thp(
            self.iter_packets_rtt().cloned().collect(),
            Duration::from_secs_f64(self.min_rtt.0),
        );
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
