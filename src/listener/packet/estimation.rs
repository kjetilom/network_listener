use super::DataPacket;
use std::time::SystemTime;
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
        GinGout {
            gin,
            gout,
            len,
            timestamp,
        }
    }

    pub fn get_dp(&self) -> (f64, f64, SystemTime) {
        (self.len / self.gin, self.gout / self.gin, self.timestamp)
    }
}

/// A sender that collects gap data points (dps) for available bandwidth estimation.
#[derive(Debug)]
pub struct PABWESender {
    pub dps: Vec<GinGout>,
}

impl PABWESender {
    pub fn new() -> Self {
        PABWESender { dps: Vec::new() }
    }

    pub fn push(&mut self, dp: GinGout) {
        self.dps.push(dp);
    }

    fn filter_gin_gacks(&mut self) -> Vec<GinGout> {
        // Get the average of the 10% smallest gin values.
        // Calculate the average gack and gin for these values:

        let mut filtered: Vec<GinGout> = self
            .dps
            .iter()
            .filter(|dp| {
                dp.gin > 0.0
                    && dp.len > 1000.0
                    && dp.len / dp.gin < crate::Settings::NEAREST_LINK_PHY_CAP / 8.0
                    && dp.len / dp.gout < crate::Settings::NEAREST_LINK_PHY_CAP / 8.0
            })
            .cloned()
            .collect();

        filtered.sort_by(|gin1, gin2| gin1.gin.partial_cmp(&gin2.gin).unwrap());

        let n = (filtered.len() as f64 * 0.1).ceil() as usize;
        let _gmin_in = filtered.iter().take(n).map(|dp| dp.gin).sum::<f64>() / n as f64;
        let gmin_out = filtered.iter().take(n).map(|dp| dp.gout).sum::<f64>() / n as f64;

        let g_max_in = gmin_out;

        let filtered: Vec<GinGout> = filtered
            .iter()
            .filter(|dp| dp.gin < g_max_in)
            .cloned()
            .collect();

        // Return the 70% smallest gin values.
        return filtered[0..(filtered.len() as f64 * 0.7).ceil() as usize].to_vec();
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
            let res = (1.0 - b) / a;
            if res > 0.0 && res < crate::Settings::NEAREST_LINK_PHY_CAP / 8.0 {
                return Some(res);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{GinGout, PABWESender};

    #[test]
    fn test_empty_sender() {
        let mut sender = PABWESender::new();
        assert!(
            sender.passive_pgm_abw().is_none(),
            "Empty sender should return None"
        );
    }

    #[test]
    fn test_zero_gin_ignored() {
        let mut sender = PABWESender::new();
        // This point has gin == 0, so it should be ignored in the regression.
        sender.push(GinGout {
            gin: 0.0,
            gout: 1.0,
            len: 1400.0,
            timestamp: std::time::SystemTime::now(),
        });
        assert!(
            sender.passive_pgm_abw().is_none(),
            "Only zero gin data should yield None"
        );
    }

    #[test]
    fn test_simple_regression() {
        let mut sender = PABWESender::new();

        let test_points = vec![
            (0.1, 0.1, 1200.0),
            (0.12, 0.15, 1200.0),
            (0.13, 0.20, 1200.0),
            (0.14, 0.25, 1200.0),
            (0.15, 0.30, 1200.0),
            (0.16, 0.35, 1200.0),
            (0.17, 0.40, 1200.0),
            (0.18, 0.45, 1200.0),
            (0.19, 0.50, 1200.0),
            (0.20, 0.55, 1200.0),
            (0.21, 0.60, 1200.0),
            (0.22, 0.65, 1200.0),
            (0.23, 0.70, 1200.0),
            (0.24, 0.75, 1200.0),
            (0.25, 0.80, 1200.0),
            (0.26, 0.85, 1200.0),
            (0.27, 0.90, 1200.0),
            (0.28, 0.95, 1200.0),
            (0.29, 1.0, 1200.0),
            (0.30, 1.05, 1200.0),
            (0.31, 1.10, 1200.0),
            (0.32, 1.15, 1200.0),
            (0.33, 1.20, 1200.0),
            (0.34, 1.25, 1200.0),
            (0.35, 1.30, 1200.0),
            (0.36, 1.35, 1200.0),
            (0.37, 1.40, 1200.0),
            (0.38, 1.45, 1200.0),
            (0.39, 1.50, 1200.0),
            (0.40, 1.55, 1200.0),
            (0.41, 1.60, 1200.0),
            (0.42, 1.65, 1200.0),
        ];

        for (gin, gout, len) in test_points {
            sender.push(GinGout {
                gin,
                gout,
                len,
                timestamp: std::time::SystemTime::now(),
            });
        }

        println!("{:?}", sender.filter_gin_gacks());

        let estimated = sender.passive_pgm_abw();
        assert!(estimated.is_some(), "Regression should produce an estimate");
        let abw = estimated.unwrap();
        // Check that the estimated available bandwidth is close to 50,
        // allowing some tolerance due to floating-point arithmetic.
        assert!(
            (abw - 11629.0).abs() < 1.0,
            "Estimated bandwidth ({}) should be approximately 11629",
            abw
        );
    }

    #[test]
    fn test_clear_function() {
        let mut sender = PABWESender::new();
        sender.push(GinGout {
            gin: 0.1,
            gout: 1.0,
            len: 1400.0,
            timestamp: std::time::SystemTime::now(),
        });
        assert!(
            !sender.dps.is_empty(),
            "Sender should have data points after push"
        );
    }
}
