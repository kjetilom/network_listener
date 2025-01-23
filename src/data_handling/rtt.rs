// use super::timeseries::Timeseries;
// use crate::listener::tracker::TcpStats;

// impl Timeseries<TcpStats> {
//     pub fn get_total_retransmissions(&self) -> u32 {
//         self.data.iter().map(|dp| dp.value.total_retransmissions).sum()
//     }

//     pub fn get_total_unique_packets(&self) -> u32 {
//         self.data.iter().map(|dp| dp.value.total_unique_packets).sum()
//     }
// }