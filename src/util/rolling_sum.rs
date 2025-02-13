use std::collections::VecDeque;

#[derive(Debug, )]
pub struct RollingSum<T> {
    window: VecDeque<T>,
    sum: T,
}

impl <T: num_traits::Num + Copy> RollingSum<T> {
    pub fn new(window_size: usize) -> Self {
        RollingSum {
            window: VecDeque::with_capacity(window_size),
            sum: T::zero(),
        }
    }

    /// Push a new value into the window and return the new sum
    pub fn push(&mut self, value: T) -> T {
        if self.window.len() == self.window.capacity() {
            let old = self.window.pop_front().unwrap();
            self.sum = self.sum - old;
        }
        self.window.push_back(value);
        self.sum = self.sum + value;
        self.sum
    }

    pub fn sum(&self) -> T {
        self.sum
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_sumf64() {
        type RollingSumF64 = RollingSum<f64>;
        let mut rs = RollingSumF64::new(3);
        assert_eq!(rs.push(1.0), 1.0);
        assert_eq!(rs.push(2.0), 3.0);
        assert_eq!(rs.push(3.0), 6.0);
        assert_eq!(rs.push(4.0), 9.0);
        assert_eq!(rs.push(5.0), 12.0);
        assert_eq!(rs.push(6.0), 15.0);
        assert_eq!(rs.push(7.0), 18.0);
    }

    #[test]
    fn test_rolling_sumu32() {
        type RollingSumU32 = RollingSum<u32>;
        let mut rs = RollingSumU32::new(3);
        assert_eq!(rs.push(1), 1);
        assert_eq!(rs.push(2), 3);
        assert_eq!(rs.push(3), 6);
        assert_eq!(rs.push(4), 9);
        assert_eq!(rs.push(5), 12);
        assert_eq!(rs.push(6), 15);
        assert_eq!(rs.push(7), 18);
    }
}
