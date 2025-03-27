use std::time::SystemTime;

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

    pub fn filter_gin_gacks(&mut self) -> Vec<GinGout> {
        // Get the average of the 10% smallest gin values.
        // Calculate the average gack and gin for these values:
        let phy_cap = crate::CONFIG.client.link_phy_cap as f64 / 8.0;

        let mut filtered: Vec<GinGout> = self
            .dps
            .iter()
            .filter(|dp| {
                dp.gin > 0.0
                    && dp.len > 1000.0 // ! Fix this, this should be set to mss in some way
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

        // Return the 70% smallest gin values. (Aka the largest len/gin values.)
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
    pub fn passive_pgm_abw(&mut self) -> (Option<f64>, Vec<GinGout>) {
        // Ensure we have some data points.
        if self.dps.is_empty() {
            return (None, Vec::new());
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

    /// Estimates the available bandwidth using a robust linear regression.
    ///
    /// This method is similar to `passive_pgm_abw`, but it uses an iterative
    /// robust least squares (IRLS) approach to minimize the influence of outliers.
    /// The available bandwidth is estimated as (1 - b) / a, where a is the slope
    /// and b is the intercept.
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

    /// Performs robust linear regression using IRLS with Huber weighting.
    /// Returns Some((slope, intercept)) on success.
    fn robust_least_squares(x: &[f64], y: &[f64]) -> Option<(f64, f64)> {
        let n = x.len();
        if n == 0 {
            return None;
        }
        let tol = 1e-6;
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

            // Update weights: if the residual is small, weight remains 1; otherwise, weight = delta/residual.
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
    use super::{GinGout, PABWESender};

    #[test]
    fn test_empty_sender() {
        let mut sender = PABWESender::new();
        assert!(
            sender.passive_pgm_abw().0.is_none(),
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
            sender.passive_pgm_abw().0.is_none(),
            "Only zero gin data should yield None"
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
