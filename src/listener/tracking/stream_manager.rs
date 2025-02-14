use crate::{stream_id::StreamKey, tracker::{Tracker, TrackerState}, Direction, PacketRegistry, ParsedPacket};
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
            sent: PacketRegistry::new(5000),
            received: PacketRegistry::new(5000),
            abw: 0.0,
            max_rtt: 0,
            min_rtt: 0,
            mean_rtt: 0,
            tcp_thput: 0.0,
            last_iperf: None,
        }
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

    pub fn record_packet(&mut self, packet: &ParsedPacket) {
        let stream_id = StreamKey::from_packet(packet);
        let result = self.streams
            .entry(stream_id)
            .or_insert_with(|| {
                Tracker::<TrackerState>::new(packet.timestamp, packet.transport.get_ip_proto())
            })
            .register_packet(packet);

        match packet.direction {
            Direction::Incoming => {
                self.received.extend(result);
            }
            Direction::Outgoing => {
                self.sent.extend(result);
            }
        };
    }

    pub fn get_latency_avg(&self) -> Option<f64> {
        self.sent.mean_rtt()
    }

    pub fn periodic(&mut self) {
        self.update_states();
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

