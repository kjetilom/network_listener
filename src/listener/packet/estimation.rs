use std::time::{Duration, SystemTime};

#[derive(Debug)]
pub struct TimeSeries<T> {
    first_timestamp: SystemTime,
    data: Vec<(Duration, T)>,
}

impl<T> TimeSeries<T> {
    pub fn new(first_timestamp: SystemTime) -> Self {
        TimeSeries {
            first_timestamp,
            data: Vec::new(),
        }
    }

    pub fn append(&mut self, timestamp: SystemTime, value: T) {
        let duration = timestamp
            .duration_since(self.first_timestamp)
            .expect("Timestamp must be after first_timestamp");
        self.data.push((duration, value));
    }

    pub fn drain_with_timestamps(
        &mut self,
    ) -> impl Iterator<Item = (SystemTime, T)> + '_ {
        let first_timestamp = self.first_timestamp;
        self.data.drain(..).map(move |(duration, value)| {
            (first_timestamp + duration, value)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yata::methods::EMA;
    use yata::prelude::*;
    use std::time::{Duration, SystemTime};
    #[test]
    fn test_ema() {
        let mut ema = EMA::new(15, &0.0).unwrap();
        assert_eq!(ema.next(&1.0), 0.125);

        let mut ema = EMA::new(15, &1.0).unwrap();
        assert_eq!(ema.next(&2.0), 1.125);
        assert_eq!(ema.next(&2.0), 1.234375);
    }


    #[test]
    fn test_timeseries() {
        let now = SystemTime::now();
        let mut ts = TimeSeries::new(now);

        ts.append(now + Duration::from_secs(1), 10);
        ts.append(now + Duration::from_secs(2), 20);
        ts.append(now + Duration::from_secs(3), 30);

        let mut drained_data = ts.drain_with_timestamps();

        let (ts1, val1) = drained_data.next().unwrap();
        let (ts2, val2) = drained_data.next().unwrap();
        let (ts3, val3) = drained_data.next().unwrap();

        assert_eq!(val1, 10);
        assert_eq!(val2, 20);
        assert_eq!(val3, 30);

        assert!(ts1 >= now);
        assert!(ts2 >= now);
        assert!(ts3 >= now);
    }

    #[test]
    fn test_empty_timeseries() {
        let now = SystemTime::now();
        let mut ts: TimeSeries<f64> = TimeSeries::new(now);
        let mut drained_data = ts.drain_with_timestamps();
        assert_eq!(drained_data.next(), None);
    }
}