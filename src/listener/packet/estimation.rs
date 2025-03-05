use std::time::SystemTime;
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

impl GinGout {
    pub fn new(gin: f64, gout: f64, len: f64, timestamp: SystemTime) -> Self {
        GinGout { gin, gout, len, timestamp }
    }

    pub fn get_dp(&self) -> (f64, f64, SystemTime) {
        (self.len / self.gin, self.gout / self.gin, self.timestamp)
    }

    pub fn from_data_packet(packet: &DataPacket) -> Option<Self> {
        let (gin, gout, timestamp) = match packet.get_gin_gout() {
            Some((gin, gout, timestamp)) => (gin, gout, timestamp),
            None => return None,
        };
        Some(GinGout { gin, gout, len: packet.total_length as f64, timestamp })
    }
}

/// A sender that collects gap data points (dps) for available bandwidth estimation.
#[derive(Debug)]
pub struct PABWESender {
    pub dps: Vec<GinGout>,
    pub window: Option<Duration>,
}

impl PABWESender {
    pub fn new(window: Option<Duration>) -> Self {
        PABWESender {
            dps: Vec::new(),
            window: window,
        }
    }

    fn push(&mut self, dp: GinGout) {
        self.dps.push(dp);
    }

    fn iter_ack_stream(&mut self, ack_stream: Vec<Vec<DataPacket>>) -> &Self {
        for ack_group in ack_stream {

            let sum_gin: f64 = ack_group.iter().map(|packet| packet.gap_last_sent.unwrap().as_secs_f64()).sum();
            let gout = ack_group.first().unwrap().gap_last_ack.unwrap().as_secs_f64();
            let sum_len: f64 = ack_group.iter().map(|packet| packet.total_length as f64).sum();
            let timestamp = ack_group.last().unwrap().ack_time.unwrap();

            self.push(
                GinGout::new(
                    sum_gin/ack_group.len() as f64,
                    gout/ack_group.len() as f64,
                    sum_len/ack_group.len() as f64,
                    timestamp,
                )
            )
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

    pub fn get_burst_thp(packets: Vec<DataPacket>, min_rtt: Duration) -> Vec<f64> {
        let bursts = PABWESender::group_bursts(packets, min_rtt);
        let mut thp = Vec::new();
        for burst in bursts {
            let mut burst_len = 0.0;
            let burst_time = burst.last().unwrap().ack_time.unwrap().duration_since(burst.first().unwrap().ack_time.unwrap()).unwrap();
            for packet in burst {
                burst_len += packet.total_length as f64;
            }
            let burst_thpt = burst_len / burst_time.as_secs_f64();
            thp.push(burst_thpt);
        }
        thp
    }

    pub fn drain(&mut self) -> Vec<GinGout> {
        self.dps.drain(..).collect()
    }

    fn group_bursts(packets: Vec<DataPacket>, min_rtt: Duration) -> Vec<Vec<DataPacket>> {
        let mut chunks = Vec::new();
        let mut chunk: Vec<DataPacket> = Vec::new();
        for packet in packets {
            if chunk.is_empty() {
                chunk.push(packet);
            } else {
                let diff = packet.sent_time.duration_since(chunk.first().unwrap().sent_time).unwrap();
                if diff < min_rtt {
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
        let _gmin_in = self.dps.iter().take(n).map(|dp| dp.gin).sum::<f64>() / n as f64;
        let gmin_out = self.dps.iter().take(n).map(|dp| dp.gout).sum::<f64>() / n as f64;

        let g_max_in = gmin_out;

        let filtered = self.dps.iter().filter(|dp| {
            dp.gin < g_max_in
        }).cloned().collect();
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
            if dp.len < 1000.0 {
                continue;
            }
            let x = dp.len / dp.gin;

            if x > crate::Settings::NEAREST_LINK_PHY_CAP/8.0 {
                continue;
            }
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
            let res = (1.0 - b) / a;
            if res > 0.0 && res < crate::Settings::NEAREST_LINK_PHY_CAP/8.0 {
                return Some(res);
            }
        }
        None
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
}