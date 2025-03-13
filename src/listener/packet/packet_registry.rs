use crate::tcp_tracker::Burst;

use super::estimation::{GinGout, PABWESender};
use std::time::SystemTime;

#[derive(Debug)]
pub struct PacketRegistry {
    rtts: Vec<(u32, SystemTime)>, // in microseconds
    burst_thput: Vec<f64>, // in bytes
    pgm_estimator: PABWESender,
    min_rtt: (f64, SystemTime),
    retransmissions: u16,
}

impl PacketRegistry {
    pub fn new() -> Self {
        PacketRegistry {
            rtts: Vec::new(),
            burst_thput: Vec::new(),
            pgm_estimator: PABWESender::new(),
            min_rtt: (f64::MAX, SystemTime::now()),
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

    pub fn passive_pgm_abw(&mut self) -> Option<f64> {
        let res = self.pgm_estimator.passive_pgm_abw();
        self.pgm_estimator.dps.clear();
        res
    }

    pub fn passive_pgm_abw_rls(&mut self) -> Option<f64> {
        let res = self.pgm_estimator.passive_pgm_abw_rls();
        self.pgm_estimator.dps.clear();
        res
    }

    pub fn take_rtts(&mut self) -> Vec<(u32, SystemTime)> {
        return std::mem::take(&mut self.rtts);
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
}

