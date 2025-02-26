use std::time::SystemTime;
use itertools::Itertools;

use super::DataPacket;
use tokio::time::Duration;
/// A structure holding a pair of gap measurements and the associated packet length.
#[derive(Debug, Clone)]
pub struct GinGout {
    pub gout: f64,
    pub gin: f64,
    pub len: f64,
    pub timestamp: SystemTime,
}

/// A sender that collects gap data points (dps) for available bandwidth estimation.
#[derive(Debug)]
pub struct PABWESender {
    pub dps: Vec<GinGout>,
    pub window: Option<Duration>,
    latest: SystemTime,
}

impl PABWESender {
    /// Creates a new, empty PABWESender.
    pub fn new(window: Option<Duration>) -> Self {
        PABWESender {
            dps: Vec::new(),
            window: window,
            latest: SystemTime::UNIX_EPOCH,
        }
    }

    /// Adds a new data point.
    fn push(&mut self, dp: GinGout) {
        self.latest = SystemTime::max(self.latest, dp.timestamp);
        self.dps.push(dp);
        if let Some(window) = self.window {
            self.dps.retain(|dp| self.latest.duration_since(dp.timestamp).unwrap() < window);
        }
    }

    fn iter_ack_stream(&mut self, ack_stream: Vec<Vec<DataPacket>>) -> &Self {
        for (ack1, ack2) in ack_stream.iter().zip(ack_stream.iter().skip(1)) {
            let send_start = ack1.first().unwrap().sent_time;
            let send_end = ack2.last().unwrap().sent_time;
            let ack_start = ack1.first().unwrap().ack_time.unwrap();
            let ack_end = ack2.last().unwrap().ack_time.unwrap();

            let gin = match send_end.duration_since(send_start) {
                Ok(duration) => duration.as_secs_f64(),
                Err(_) => 0.0,
            };
            let gout = match ack_end.duration_since(ack_start) {
                Ok(duration) => duration.as_secs_f64(),
                Err(_) => 0.0,
            };

            if gin == 0.0 || gout == 0.0 {
                continue;
            }

            // Get average packet length
            let mut len = 0.0 as f64;
            for (packet1, packet2) in ack1.iter().zip(ack2.iter()) {
                len += packet1.total_length as f64;
                len += packet2.total_length as f64;
            }
            let dplen = (ack1.len() + ack2.len()) as f64 - 1.0;

            len /= dplen + 1.0;

            self.push(GinGout { gout: gout/dplen, gin: gin/dplen, len, timestamp: send_start});
        }
        self
    }

    fn group_acks(packets: Vec<DataPacket>) -> Vec<Vec<DataPacket>> {
        let mut chunks = Vec::new();
        let mut chunk = Vec::new();
        for packet in packets {
            if chunk.is_empty() {
                chunk.push(packet);
            } else {
                if packet.ack_time.unwrap() == chunk.last().unwrap().ack_time.unwrap() {
                    chunk.push(packet);
                } else {
                    chunks.push(chunk);
                    chunk = Vec::new();
                    chunk.push(packet);
                }
            }
        }
        if chunk.len() > 0 {
            chunks.push(chunk);
        }
        chunks
    }

    pub fn iter_packets(&mut self, packets: &Vec<DataPacket>) -> &Self {
        let ack_stream = PABWESender::group_acks(packets.clone());
        self.iter_ack_stream(ack_stream)
    }

    fn filter_gin_gacks(&mut self) -> Vec<GinGout> {
        // Get the average of the 10% smallest gin values.
        // Calculate the average gack and gin for these values:
        self.dps.sort_by(
            |gin1, gin2| gin1.gin.partial_cmp(&gin2.gin).unwrap()
        );

        let n = (self.dps.len() as f64 * 0.1).ceil() as usize;
        let gmin_in = self.dps.iter().take(n).map(|dp| dp.gin).sum::<f64>() / n as f64;
        let gmin_out = self.dps.iter().take(n).map(|dp| dp.gout).sum::<f64>() / n as f64;

        println!("Gmin in: {}, Gmin out: {}", gmin_in, gmin_out);

        let g_max_in = gmin_out;

        let filtered = self.dps.iter().filter(|dp| {
            dp.gin < g_max_in && dp.gout/dp.gin < 5.0 && dp.len > 1300.0 && dp.len < 1600.0
        }).cloned().collect();
        // Return the middle 80% of the data points.
        filtered
    }

    /// Estimates the available bandwidth using a linear regression.
    ///
    /// For each data point, we define:
    ///   x = len / gin   (representing the effective packet size per input gap)
    ///   y = gout / gin   (the gap ratio)
    ///
    /// The regression line is computed over all points, and the available
    /// bandwidth is estimated as (1 - b) / a, where a is the slope and b is the intercept.
    pub fn passive_pgm_abw(&mut self) -> Option<f64> {
        // Ensure we have some data points.
        if self.dps.is_empty() {
            return None;
        }

        let dps = self.filter_gin_gacks();

        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_x2 = 0.0;

        // Process each data point, skipping any with a zero gin (to avoid division by zero).
        let mut count = 0;
        for dp in &dps {
            if dp.gin == 0.0 {
                continue;
            }
            let x = dp.len / dp.gin;
            let y = dp.gout / dp.gin;
            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_x2 += x * x;
            count += 1;
        }

        if count == 0 {
            return None;
        }

        let n = count as f64;
        let numerator = n * sum_xy - sum_x * sum_y;
        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator.abs() < f64::EPSILON {
            return None;
        }
        let a = numerator / denominator;
        let b = (sum_y - a * sum_x) / n;

        if a.abs() > f64::EPSILON {
            Some((1.0 - b) / a)
        } else {
            None
        }
    }
}


#[cfg(test)]
mod tests {
    use super::{PABWESender, GinGout};

    #[test]
    fn test_empty_sender() {
        let mut sender = PABWESender::new(None);
        assert!(sender.passive_pgm_abw().is_none(), "Empty sender should return None");
    }

    #[test]
    fn test_zero_gin_ignored() {
        let mut sender = PABWESender::new(None);
        // This point has gin == 0, so it should be ignored in the regression.
        sender.push(GinGout { gin: 0.0, gout: 1.0, len: 1400.0, timestamp: std::time::SystemTime::now() });
        assert!(sender.passive_pgm_abw().is_none(), "Only zero gin data should yield None");
    }

    #[test]
    fn test_simple_regression() {
        let mut sender = PABWESender::new(None);
        // We'll create data points that ideally lie on a line defined by:
        // y = a * x + b, with a = 0.01 and b = 0.5.
        // According to our estimation, available bandwidth (abw) is:
        //   abw = (1 - b) / a = (1 - 0.5) / 0.01 = 50.
        //
        // For a given point, let:
        //   x = len / gin, and y = gout / gin = 0.01 * x + 0.5.
        // Then, for an arbitrary gin and len, set gout = y * gin.
        let test_points = vec![
            (0.1, 100.0),
            (0.12, 100.0),
            (0.15, 100.0),
            (0.2, 100.0),
            (0.25, 100.0),
        ];

        for (gin, len) in test_points {
            let x = len / gin;
            let y = 0.01 * x + 0.5;
            let gout = y * gin;
            sender.push(GinGout { gin, len, gout, timestamp: std::time::SystemTime::now() });
        }

        let estimated = sender.passive_pgm_abw();
        assert!(estimated.is_some(), "Regression should produce an estimate");
        let abw = estimated.unwrap();
        // Check that the estimated available bandwidth is close to 50,
        // allowing some tolerance due to floating-point arithmetic.
        assert!((abw - 50.0).abs() < 1.0, "Estimated bandwidth ({}) should be approximately 50", abw);
    }

    #[test]
    fn test_clear_function() {
        let mut sender = PABWESender::new(None);
        sender.push(GinGout { gin: 0.1, gout: 1.0, len: 1400.0, timestamp: std::time::SystemTime::now() });
        assert!(!sender.dps.is_empty(), "Sender should have data points after push");
    }

    #[test]
    fn test_window() {
        let mut sender = PABWESender::new(Some(std::time::Duration::from_secs(2)));
        let tstamp = std::time::SystemTime::now();
        sender.push(GinGout { gin: 0.1, gout: 1.0, len: 1400.0, timestamp: tstamp });
        assert_eq!(sender.latest, tstamp, "First timestamp should be set after push");
        assert_eq!(sender.dps.len(), 1, "Sender should have one data point after push");
        sender.push(GinGout { gin: 0.1, gout: 1.0, len: 1400.0, timestamp: tstamp + std::time::Duration::from_secs(3) });
        sender.push(GinGout { gin: 0.1, gout: 1.0, len: 1400.0, timestamp: tstamp + std::time::Duration::from_secs(3) });
        assert_eq!(sender.dps.len(), 2, "Sender should have two data point after push");
        sender.push(GinGout { gin: 0.1, gout: 1.0, len: 1400.0, timestamp: tstamp + std::time::Duration::from_secs(4) });
        assert_eq!(sender.dps.len(), 3, "Sender should have three data points after push");
        sender.push(GinGout { gin: 0.1, gout: 1.0, len: 1400.0, timestamp: tstamp + std::time::Duration::from_secs(5) });
        assert_eq!(sender.dps.len(), 2, "Sender should have two data points after push");
        assert_eq!(sender.latest, tstamp + std::time::Duration::from_secs(5), "Latest timestamp should be updated after push");
        assert_eq!(sender.dps.last().unwrap().timestamp, tstamp + std::time::Duration::from_secs(5), "Last timestamp should be the latest");
    }
}