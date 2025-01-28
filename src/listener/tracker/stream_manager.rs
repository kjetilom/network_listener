use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use std::collections::HashMap;

use super::super::packet::packet_builder::ParsedPacket;
use super::super::tracker::stream_id::StreamKey;
use super::super::tracker::tracker::{Tracker, TrackerState};
// Replace HashMap with DashMap
#[derive(Debug)]
pub struct StreamManager {
    // HashMap for all streams
    streams: HashMap<StreamKey, Tracker<TrackerState>>,
}

impl StreamManager {
    pub fn default() -> Self {
        StreamManager {
            streams: HashMap::new(),
        }
    }

    pub fn record_ip_packet(&mut self, packet: &ParsedPacket) {
        let stream_id = StreamKey::from_packet(&packet);
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
                        latencies.push(rtt);
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

    pub fn periodic(&mut self) {
        self.update_states();
        for (stream_id, tracker) in self.streams.iter_mut() {
            match tracker.state {
                TrackerState::Tcp(ref mut tcp_tracker) => {
                    println!("{}: {}", stream_id, tcp_tracker.stats.smoothed_rtt.unwrap_or(0.0));
                }
                _ => {
                    println!("{}", stream_id);
                }
            }
        }
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
