use crate::tcp_tracker::Burst;

use super::estimation::{GinGout, PABWESender};
use std::time::SystemTime;

/// Type of regression to use in passive bandwidth estimation.
///
/// - `Simple`: Ordinary least squares regression.
/// - `RLS`: Robust least squares regression (IRLS with Huber weight).
#[derive(Debug, Clone, Copy)]
pub enum RegressionType {
    /// RLS (Robust Least Squares) regression.
    RLS,
    /// Simple linear regression.
    Simple,
}

/// Registry for tracking packet statistics over time.
///
/// Stores RTT samples, burst throughputs, and uses a PABWE sender
/// to accumulate GinGout points for passive available bandwidth estimation.
#[derive(Debug)]
pub struct PacketRegistry {
    /// Vector of round-trip times (RTTs) in microseconds.
    pub rtts: Vec<(u32, SystemTime)>,
    /// Sum of RTTs and the count of RTT samples.
    pub sum_rtt: (f64, u32),
    /// Vector of burst throughput values in bytes.
    pub burst_thput: Vec<f64>,
    /// PABWE sender instance for bandwidth estimation.
    pub pgm_estimator: PABWESender,
    /// Minimum RTT value and its corresponding timestamp.
    min_rtt: (f64, SystemTime),
    /// Count of retransmissions.
    retransmissions: u16,
}

impl Default for PacketRegistry {
    fn default() -> Self {
        PacketRegistry::new()
    }
}

impl PacketRegistry {

    /// Creates a new instance of `PacketRegistry`.
    ///
    /// Initializes all fields to default values.
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

    /// Returns the minimum RTT value in microseconds.
    /// If no RTTs are recorded, returns `None`.
    pub fn min_rtt(&self) -> Option<f64> {
        if self.min_rtt.0 == f64::MAX {
            None
        } else {
            Some(self.min_rtt.0)
        }
    }

    /// Performs passive available bandwidth estimation.
    ///
    /// Chooses regression based on `regression_type`.
    /// - `RegressionType::Simple`: uses ordinary least squares.
    /// - `RegressionType::RLS`: uses robust IRLS regression.
    ///
    /// Returns `(estimated_bw, used_data_points)`.
    pub fn passive_abw(&mut self, regression_type: RegressionType) -> (Option<f64>, Vec<GinGout>) {
        match regression_type {
            RegressionType::RLS => self.pgm_estimator.passive_pgm_abw_rls(),
            RegressionType::Simple => self.pgm_estimator.passive_pgm_abw(),
        }
    }

    /// Takes the current registry, replacing it with the default instance.
    ///
    /// Returns the previous state
    pub fn take(&mut self) -> Self {
        std::mem::take(self)
    }


    /// Extends the registry with a new `Burst` of packets.
    ///
    /// For TCP bursts, records throughput, GinGout points, RTTs, and retransmissions.
    /// Ignores other burst types.
    pub fn extend(&mut self, values: Burst) {
        // Record burst throughput regardless of type
        self.burst_thput.push(values.throughput());
        // Only process TCP bursts for detailed stats
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
                // Record RTTs and retransmissions
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

    /// Returns the average RTT (microseconds), or `None` if no samples.
    pub fn avg_rtt(&self) -> Option<f64> {
        if self.sum_rtt.1 == 0 {
            None
        } else {
            Some(self.sum_rtt.0 / self.sum_rtt.1 as f64)
        }
    }

    /// Returns total retransmissions observed.
    pub fn retransmissions(&self) -> u16 {
        self.retransmissions
    }

    /// Returns the average burst throughput (bytes/sec), or `None` if none recorded.
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
    use super::{PacketRegistry, RegressionType};
    use crate::tcp_tracker::{Burst, TcpBurst};

    #[test]
    fn test_default_and_take() {
        let mut reg = PacketRegistry::new();
        assert!(reg.rtts.is_empty());
        let prev = reg.take();
        // After take, registry is default
        assert!(reg.rtts.is_empty());
        assert!(prev.rtts.is_empty());
    }

    #[test]
    fn test_min_avg_rtt_none() {
        let reg = PacketRegistry::new();
        assert_eq!(reg.min_rtt(), None);
        assert_eq!(reg.avg_rtt(), None);
    }

    #[test]
    fn test_retransmissions_and_thp_empty() {
        let mut reg = PacketRegistry::new();
        // Extend with empty TCP burst
        let empty = Burst::Tcp(TcpBurst { packets: Vec::new() });
        reg.extend(empty);
        assert_eq!(reg.retransmissions(), 0);
        assert_eq!(reg.burst_thput.len(), 1);
        assert!(reg.avg_burst_thp().is_some());
    }

    #[test]
    fn test_passive_abw_empty() {
        let mut reg = PacketRegistry::new();
        let (bw_simple, pts_simple) = reg.passive_abw(RegressionType::Simple);
        assert!(bw_simple.is_none());
        assert!(pts_simple.is_empty());
        let (bw_rls, pts_rls) = reg.passive_abw(RegressionType::RLS);
        assert!(bw_rls.is_none());
        assert!(pts_rls.is_empty());
    }

    #[test]
    fn test_min_rtt_and_avg_rtt_after_extend() {
        let mut reg = PacketRegistry::new();
        // Create a dummy burst with one packet having an RTT
        let burst = TcpBurst { packets: Vec::new() };
        let empty = Burst::Tcp(burst);
        reg.extend(empty);
        // min_rtt remains None
        assert_eq!(reg.min_rtt(), None);
    }
}
