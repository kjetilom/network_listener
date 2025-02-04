use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use tokio::time::Instant;
use std::collections::HashMap;

use super::super::packet::ParsedPacket;
use super::super::tracking::stream_id::StreamKey;
use super::super::tracking::tracker::{Tracker, TrackerState};
// Replace HashMap with DashMap
#[derive(Debug)]
pub struct StreamManager {
    // HashMap for all streams
    streams: HashMap<StreamKey, Tracker<TrackerState>>,
    data_in: u32,
    data_out: u32,
    max_in: u32,
    max_out: u32,
    abw: f64,
    pub last_iperf: Option<Instant>,
}

impl StreamManager {
    pub fn default() -> Self {
        StreamManager {
            streams: HashMap::new(),
            data_in: 0,
            data_out: 0,
            max_in: 0,
            max_out: 0,
            abw: 0.0,
            last_iperf: None,
        }
    }

    pub fn contains_udp_tcp(&self) -> bool {
        for (_stream_id, tracker) in self.streams.iter() {
            match tracker.protocol {
                IpNextHeaderProtocols::Udp => return true,
                IpNextHeaderProtocols::Tcp => return true,
                _ => {}
            }
        }
        false
    }



    pub fn record_iperf_result(&mut self, bps: f64) {
        // Check if in out is very different
        self.last_iperf = Some(Instant::now());
        self.abw = bps;
    }

    pub fn get_abw(&self) -> f64 {
        self.abw
    }

    pub fn record_ip_packet(&mut self, packet: &ParsedPacket) {
        match packet.direction {
            super::super::packet::Direction::Incoming => {
                self.data_in += packet.total_length;
            }
            super::super::packet::Direction::Outgoing => {
                self.data_out += packet.total_length;
            }
        }
        let stream_id = StreamKey::from_packet(packet);
        self.streams.entry(stream_id)
            .or_insert_with(|| Tracker::<TrackerState>::new(
                packet.timestamp,
                packet.transport.get_ip_proto()
            ))
            .register_packet(packet);
    }

    pub fn get_latency_avg(&self) -> Option<f64> {
        let mut latencies = Vec::new();
        for (_stream_id, tracker) in self.streams.iter() {
            match tracker.state {
                TrackerState::Tcp(ref tcp_tracker) => {
                    if let Some(rtt) = tcp_tracker.stats.smoothed_rtt {
                        if rtt > 0.0 {
                            latencies.push(rtt);
                        }
                    }
                }
                _ => {}
            }
        }
        if latencies.is_empty() {
            None
        } else {
            Some(latencies.iter().sum::<f64>() / latencies.len() as f64)
        }
    }

    pub fn get_rt_in_out(&self) -> (u32, u32) {
        let mut rt_in = 0;
        let mut rt_out = 0;
        for (_stream_id, tracker) in self.streams.iter() {
            match tracker.state {
                TrackerState::Tcp(ref tcp_tracker) => {
                    rt_in += tcp_tracker.stats.retrans_in;
                    rt_out += tcp_tracker.stats.retrans_out;
                }
                _ => {}
            }
        }
        (rt_in, rt_out)
    }

    pub fn get_in_out(&self) -> (u32, u32) {
        (self.max_in, self.max_out) // REMOVE THIS
    }

    pub fn periodic(&mut self) {
        self.update_states();
        if self.data_in > self.max_in {
            self.max_in = self.data_in;
        }
        if self.data_out > self.max_out {
            self.max_out = self.data_out;
        }
        self.data_in = 0;
        self.data_out = 0;
    }

    fn update_states(&mut self) {
        let mut ids_to_remove: Vec<StreamKey> = Vec::new();

        for (stream_id, tracker) in self.streams.iter_mut() {
            if tracker.last_registered.elapsed().unwrap().as_secs()
                >= super::super::Settings::TCP_STREAM_TIMEOUT.as_secs()
            {
                ids_to_remove.push(*stream_id);
                continue;
            }
        }
        self.streams.retain(|k, _| !ids_to_remove.contains(k));
    }

    pub fn take_streams(&mut self, keys: Vec<StreamKey>) -> Vec<Tracker<TrackerState>> {
        let mut taken = Vec::new();

        for key in keys {
            if let Some(tracker) = self.streams.remove(&key) {
                taken.push(tracker);
            }
        }
        taken
    }

    pub fn get_tcp_streams(&self) -> Vec<&Tracker<TrackerState>> {
        self.get_streams(IpNextHeaderProtocols::Tcp)
    }

    pub fn get_streams(&self, protocol: IpNextHeaderProtocol) -> Vec<&Tracker<TrackerState>> {
        self.streams
            .values()
            .filter(|t| t.protocol == protocol)
            .collect()
    }
}
