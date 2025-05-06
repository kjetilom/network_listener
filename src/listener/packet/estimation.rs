use std::time::SystemTime;

// Minimum payload size threshold: MTU (1500 bytes) minus maximum header sizes (IP+Ethernet+TCP).
const MIN_PAYLOAD_SIZE: f64 = 1362.0;

/// A structure holding a pair of gap measurements and the associated packet length.
#[derive(Debug, Clone)]
pub struct GinGout {
    /// Gap between this packet's ack and the previous ack (s).
    pub gout: f64,
    /// Gap between this packet's send and the previous send (s).
    pub gin: f64,
    /// Packet payload length (bytes).
    pub len: f64,
    /// Number of packets acknowledged by this ack. (cumulative ack number)
    pub num_acked: u8,
    /// Timestamp when the ack was observed.
    pub timestamp: SystemTime,
}

impl GinGout {
    /// Computes packet metrics.
    ///
    /// Returns a tuple `(x, y, timestamp)` where:
    /// - `x = len / gin` (bytes per input gap)
    /// - `y = gout / gin` (output-to-input gap ratio)
    /// - `timestamp`: original timestamp
    pub fn get_dp(&self) -> (f64, f64, SystemTime) {
        (self.len / self.gin, self.gout / self.gin, self.timestamp)
    }
}

/// Sender that accumulates `GinGout` data points for passive bandwidth estimation.
#[derive(Debug)]
pub struct PABWESender {
    pub dps: Vec<GinGout>,
}

impl PABWESender {
    pub fn new() -> Self {
        PABWESender { dps: Vec::new() }
    }

    /// Appends a new data point to the collection.
    pub fn push(&mut self, dp: GinGout) {
        self.dps.push(dp);
    }

    /// Filters data points based on minimum payload, nonzero gaps, and link capacity.
    ///
    /// Steps:
    /// 1. Discard any `dp` where `gin == 0`, `len < MIN_PAYLOAD_SIZE`, or ratio constraints exceed physical capacity.
    /// 2. Sort remaining by `gin` ascending.
    /// 3. Compute average of the smallest 10% of `gin` and corresponding `gout`.
    /// 4. Retain only points with `gin < average_gout`.
    ///
    /// # Returns
    /// A vector of `GinGout` that passed all filters.
    pub fn filter_gin_gacks(&mut self) -> Vec<GinGout> {
        // Convert bit to byte.
        let phy_cap = crate::CONFIG.client.link_phy_cap as f64 / 8.0;

        let mut filtered: Vec<GinGout> = self
            .dps
            .iter()
            .filter(|dp| {
                dp.gin > 0.0
                    && dp.len >= MIN_PAYLOAD_SIZE
                    && dp.len / dp.gin < phy_cap
                    && dp.len / dp.gout < phy_cap
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

        return filtered;
    }

    /// Estimates available bandwidth via ordinary least squares regression.
    ///
    /// Returns `(Some(bw), used_points)` if estimation succeeded and bandwidth in bytes/sec;
    /// otherwise `(None, used_points)`.
    ///
    /// ! The used_points are should be removed in production code.
    /// ! They are returned to be pushed to the database.
    pub fn passive_pgm_abw(&mut self) -> (Option<f64>, Vec<GinGout>) {
        // Ensure we have some data points.
        if self.dps.is_empty() {
            return (None, Vec::new());
        }

        let dps = self.filter_gin_gacks();

        let (mut sum_x, mut sum_y, mut sum_xy, mut sum_x2, mut count) = (0.0, 0.0, 0.0, 0.0, 0);

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
            return (None, dps);
        }

        let n = count as f64;
        let numerator = n * sum_xy - sum_x * sum_y;
        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator.abs() < f64::EPSILON {
            return (None, dps);
        }
        let a = numerator / denominator;
        let b = (sum_y - a * sum_x) / n;

        if a.abs() > f64::EPSILON {
            let res = (1.0 - b) / a;
            if res > 0.0 && res < crate::CONFIG.client.link_phy_cap as f64 / 8.0 {
                return (Some(res), dps);
            }
        }
        (None, dps)
    }

    /// Estimates available bandwidth using robust linear regression (IRLS with Huber weighting).
    pub fn passive_pgm_abw_rls(&mut self) -> (Option<f64>, Vec<GinGout>) {
        if self.dps.is_empty() {
            return (None, Vec::new());
        }

        let dps = self.filter_gin_gacks();
        let mut xs: Vec<f64> = Vec::new();
        let mut ys: Vec<f64> = Vec::new();

        for dp in &dps {
            if dp.gin.abs() < f64::EPSILON {
                continue;
            }
            let x = dp.len / dp.gin;
            let y = dp.gout / dp.gin;
            xs.push(x);
            ys.push(y);
        }

        if xs.is_empty() {
            return (None, dps);
        }

        // Perform robust regression.
        let (a, b) = match Self::robust_least_squares(&xs, &ys) {
            Some((a, b)) => (a, b),
            None => return (None, dps),
        };

        if a.abs() < f64::EPSILON {
            return (None, dps);
        }

        // Calculate the result as (1 - b) / a.
        let res = (1.0 - b) / a;
        if res > 0.0 && res < crate::CONFIG.client.link_phy_cap as f64 / 8.0 {
            (Some(res), dps)
        } else {
            (None, dps)
        }
    }

    /// Performs IRLS-based robust least squares with Huber weights.
    ///
    /// Returns `Some((slope, intercept))` or `None` on failure.
    fn robust_least_squares(x: &[f64], y: &[f64]) -> Option<(f64, f64)> {
        let n = x.len();
        if n == 0 {
            return None;
        }
        let tol = 1e-4;
        let max_iter = 100;
        let mut weights = vec![1.0; n];
        let mut a = 0.0;
        let mut b = 0.0;

        for _ in 0..max_iter {
            // Weighted sums for the regression.
            let (mut sum_w, mut sum_wx, mut sum_wy, mut sum_wxx, mut sum_wxy) =
                (0.0, 0.0, 0.0, 0.0, 0.0);
            for i in 0..n {
                let w = weights[i];
                let xi = x[i];
                let yi = y[i];
                sum_w += w;
                sum_wx += w * xi;
                sum_wy += w * yi;
                sum_wxx += w * xi * xi;
                sum_wxy += w * xi * yi;
            }

            let denominator = sum_w * sum_wxx - sum_wx * sum_wx;
            if denominator.abs() < f64::EPSILON {
                return None;
            }
            let new_a = (sum_w * sum_wxy - sum_wx * sum_wy) / denominator;
            let new_b = (sum_wy - new_a * sum_wx) / sum_w;

            // Check for convergence.
            if (new_a - a).abs() < tol && (new_b - b).abs() < tol {
                a = new_a;
                b = new_b;
                break;
            }
            a = new_a;
            b = new_b;

            // Compute absolute residuals.
            let mut residuals: Vec<f64> = x
                .iter()
                .zip(y.iter())
                .map(|(xi, yi)| (yi - (a * xi + b)).abs())
                .collect();

            // Compute the median of the residuals.
            residuals.sort_by(|r1, r2| r1.partial_cmp(r2).unwrap());
            let median = if n % 2 == 1 {
                residuals[n / 2]
            } else {
                (residuals[n / 2 - 1] + residuals[n / 2]) / 2.0
            };
            // Set Huber threshold.
            let mut delta = 1.345 * median;
            if delta < tol {
                delta = tol;
            }

            // Update weights
            for i in 0..n {
                let res = (y[i] - (a * x[i] + b)).abs();
                weights[i] = if res <= delta { 1.0 } else { delta / res };
            }
        }
        Some((a, b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn test_get_dp() {
        let t = SystemTime::now();
        let gg = GinGout {
            gin: 2.0,
            gout: 4.0,
            len: 1000.0,
            num_acked: 1,
            timestamp: t,
        };
        let (x, y, ts) = gg.get_dp();
        assert_eq!(x, 500.0);
        assert_eq!(y, 2.0);
        assert_eq!(ts, t);
    }

    #[test]
    fn test_filter_empty() {
        let mut s = PABWESender::new();
        let filtered = s.filter_gin_gacks();
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_filter_small_payload() {
        let mut s = PABWESender::new();
        s.push(GinGout {
            gin: 1.0,
            gout: 1.0,
            len: 100.0,
            num_acked: 1,
            timestamp: SystemTime::now(),
        });
        let filtered = s.filter_gin_gacks();
        assert!(
            filtered.is_empty(),
            "Packets below MIN_PAYLOAD_SIZE should be dropped"
        );
    }

    #[test]
    fn test_robust_least_squares_simple() {
        let xs = [1.0, 2.0, 3.0];
        let ys = [2.0, 4.0, 6.0];
        if let Some((a, b)) = PABWESender::robust_least_squares(&xs, &ys) {
            assert!((a - 2.0).abs() < 1e-6);
            assert!((b - 0.0).abs() < 1e-6);
        } else {
            panic!("Expected Some((2.0,0.0))");
        }
    }

    #[test]
    fn test_empty_abw_methods() {
        let mut s = PABWESender::new();
        assert!(s.passive_pgm_abw().0.is_none());
        assert!(s.passive_pgm_abw_rls().0.is_none());
    }
}
