use crate::tcp_tracker::Burst;

use super::estimation::{GinGout, PABWESender};
use super::DataPacket;
use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
    time::{Duration, SystemTime},
};

#[derive(Debug)]
pub struct PacketRegistry {
    packets: VecDeque<DataPacket>,
    rtts: Vec<(u32, SystemTime)>, // in microseconds
    burst_thput: Vec<f64>, // in bytes
    pgm_estimator: PABWESender,
    min_rtt: (f64, SystemTime),
    sum_data: u32,
    retransmissions: u16,
}

impl PacketRegistry {
    pub fn new(size: usize) -> Self {
        PacketRegistry {
            packets: VecDeque::with_capacity(size),
            rtts: Vec::new(),
            burst_thput: Vec::new(),
            pgm_estimator: PABWESender::new(),
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
            self.min_rtt = (
                min_rtt.rtt.unwrap().as_secs_f64(),
                min_rtt.ack_time.unwrap(),
            );
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
        let res = self.pgm_estimator.passive_pgm_abw();
        self.pgm_estimator.dps.clear();
        res
    }

    pub fn take_rtts(&mut self) -> Vec<(u32, SystemTime)> {
        return std::mem::take(&mut self.rtts);
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

    pub fn extend(&mut self, values: Burst) {
        // This is a vector of packets acked by one ack
        self.burst_thput.push(values.throughput());
        match values {
            Burst::Tcp(burst) => {
                let mut last_ack = None;
                for ack in &burst.packets {
                    if last_ack.is_some() {
                        let (gin, gout, total_length) =
                            match ack.get_gin_gout_len(last_ack.unwrap()) {
                                Some((gin, gout, total_length)) => (gin, gout, total_length),
                                None => continue,
                            };
                        self.pgm_estimator.push(GinGout {
                            gin: gin / ack.len() as f64,
                            gout: gout / ack.len() as f64,
                            len: total_length as f64 / ack.len() as f64,
                            timestamp: ack.ack_time,
                        });
                    }
                    last_ack = Some(ack.ack_time);
                }
                self.rtts.extend(burst.iter().map(|p| (p.rtt.unwrap().as_micros() as u32, p.sent_time)));

            }
            _ => {}
        }
    }

    pub fn avg_rtt(&self) -> Option<f64> {
        let rtts: Vec<f64> = self.rtts.iter().map(|rtt| rtt.0 as f64).collect();
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
        if self.burst_thput.is_empty() {
            None
        } else {
            Some(self.burst_thput.iter().sum::<f64>() / self.burst_thput.len() as f64)
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
