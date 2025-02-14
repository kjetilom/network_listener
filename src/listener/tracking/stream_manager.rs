use crate::{stream_id::StreamKey, tracker::{Tracker, TrackerState}, PacketRegistry, ParsedPacket};
use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use std::collections::HashMap;
use tokio::time::Instant;

/// StreamManager is a struct that keeps track of all streams and their states.
#[derive(Debug)]
pub struct StreamManager {
    // HashMap for all streams
    streams: HashMap<StreamKey, Tracker<TrackerState>>,
    sent: PacketRegistry,
    received: PacketRegistry,
    data_in: u32,
    data_out: u32,
    max_in: u32,
    max_out: u32,
    abw: f64,
    max_rtt: i64,
    min_rtt: i64,
    mean_rtt: i64,
    tcp_thput: f64,
    pub last_iperf: Option<Instant>,
}

impl StreamManager {
    pub fn default() -> Self {
        StreamManager {
            streams: HashMap::new(),
            sent: PacketRegistry::new(1000),
            received: PacketRegistry::new(1000),
            data_in: 0,
            data_out: 0,
            max_in: 0,
            max_out: 0,
            abw: 0.0,
            max_rtt: 0,
            min_rtt: 0,
            mean_rtt: 0,
            tcp_thput: 0.0,
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

    pub fn record_iperf_result(&mut self, bps: f64, stream: Option<&crate::IperfStream>) {
        // Check if in out is very different
        self.last_iperf = Some(Instant::now());
        self.tcp_thput = bps;
        if let Some(stream) = stream {
            self.max_rtt = stream.sender.max_rtt.unwrap_or(0);
            self.min_rtt = stream.sender.min_rtt.unwrap_or(0);
            self.mean_rtt = stream.sender.mean_rtt.unwrap_or(0);
        }
    }

    pub fn get_abw(&self) -> f64 {
        self.abw
    }

    pub fn tcp_thput(&self) -> f64 {
        self.tcp_thput
    }

    pub fn record_ip_packet(&mut self, packet: &ParsedPacket) {
        match packet.direction {
            super::super::packet::Direction::Incoming => {
                self.data_in += packet.total_length as u32;
            }
            super::super::packet::Direction::Outgoing => {
                self.data_out += packet.total_length as u32;
            }
        }
        let stream_id = StreamKey::from_packet(packet);
        self.streams
            .entry(stream_id)
            .or_insert_with(|| {
                Tracker::<TrackerState>::new(packet.timestamp, packet.transport.get_ip_proto())
            })
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

    pub fn get_rts(&self) -> u32 {
        let mut rts = 0;
        for (_stream_id, tracker) in self.streams.iter() {
            match tracker.state {
                TrackerState::Tcp(ref tcp_tracker) => {
                    rts += tcp_tracker.stats.rts;
                }
                _ => {}
            }
        }
        rts
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
                >= crate::Settings::TCP_STREAM_TIMEOUT.as_secs()
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

