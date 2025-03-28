use crate::tcp_tracker::Burst;

use super::estimation::{GinGout, PABWESender};
use std::time::SystemTime;

#[derive(Debug)]
pub struct PacketRegistry {
    pub rtts: Vec<(u32, SystemTime)>,
    pub sum_rtt: (f64, u32),
    pub burst_thput: Vec<f64>,        // in bytes
    pub pgm_estimator: PABWESender,
    min_rtt: (f64, SystemTime),
    retransmissions: u16,
}

impl Default for PacketRegistry {
    fn default() -> Self {
        PacketRegistry::new()
    }
}

impl PacketRegistry {
    pub fn new() -> Self {
        PacketRegistry {
            rtts: Vec::new(),
            sum_rtt: (0.0, 0),
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

    pub fn passive_abw(&mut self, robust: bool) -> (Option<f64>, Vec<GinGout>) {
        match robust {
            true => self.pgm_estimator.passive_pgm_abw_rls(),
            false => self.pgm_estimator.passive_pgm_abw(),
        }
    }

    pub fn take(&mut self) -> Self {
        std::mem::take(self) // Reset the registry
    }


    /// Extend the packet registry with a new burst of packets grouped by ack.
    ///
    /// Burst: Vec<AckedPackets>
    /// AckedPackets: Vec<DataPacket>
    ///
    /// Stores information about the packets in the burst, such as:
    /// - RTT
    /// - Gin/Gout
    /// - Retransmissions
    pub fn extend(&mut self, values: Burst) {
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
                            num_acked: ack.len() as u8,
                            timestamp: ack.ack_time,
                        });
                    }
                    last_ack = Some(ack.ack_time);
                }
                burst.iter().for_each(|p| {
                    if p.rtt.is_some() {
                        self.min_rtt = (
                            self.min_rtt.0.min(p.rtt.unwrap().as_micros() as f64),
                            p.sent_time,
                        );
                        self.sum_rtt.0 += p.rtt.unwrap().as_micros() as f64;
                        self.sum_rtt.1 += 1;
                        self.retransmissions += p.retransmissions as u16;
                        self.rtts.push((p.rtt.unwrap().as_micros() as u32, p.sent_time));
                    }
                });
            }
            _ => {}
        }
    }

    pub fn avg_rtt(&self) -> Option<f64> {
        if self.sum_rtt.1 == 0 {
            None
        } else {
            Some(self.sum_rtt.0 / self.sum_rtt.1 as f64)
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


#[cfg(test)]
mod tests {
    use crate::tcp_tracker::TcpBurst;

    #[test]
    fn test_extend_empty() {
        let mut registry = super::PacketRegistry::new();
        let burst = super::Burst::Tcp(TcpBurst {
            packets: Vec::new(),
        });
        registry.extend(burst);
        assert_eq!(registry.burst_thput.len(), 1);
    }
}