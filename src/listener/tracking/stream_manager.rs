use crate::{
    stream_id::StreamKey, tracker::{Tracker, TrackerState}, DataPacket, PacketRegistry, PacketType, ParsedPacket
};
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
    tcp_thput: f64,
    pub last_iperf: Option<Instant>,
}

impl StreamManager {
    pub fn default() -> Self {
        StreamManager {
            streams: HashMap::new(),
            sent: PacketRegistry::new(5000),
            received: PacketRegistry::new(5000),
            tcp_thput: 0.0,
            last_iperf: None,
        }
    }

    pub fn record_iperf_result(&mut self, bps: f64, stream: Option<&crate::IperfStream>) {
        // Check if in out is very different
        self.last_iperf = Some(Instant::now());
        self.tcp_thput = bps;
    }

    pub fn drain_rtts(&mut self) -> Vec<DataPacket> {
        self.sent.get_rtts()
    }

    pub fn tcp_thput(&self) -> f64 {
        self.tcp_thput
    }

    pub fn get_loss(&self) -> f64 {
        self.sent.loss()
    }

    pub fn abw(&mut self) -> f64 {
        self.sent.passive_pgm_abw().unwrap_or(0.0)
    }

    pub fn record_packet(&mut self, packet: &ParsedPacket) {
        let stream_id = StreamKey::from_packet(packet);
        let result = self
            .streams
            .entry(stream_id)
            .or_insert_with(|| {
                Tracker::<TrackerState>::new(packet.timestamp, packet.transport.get_ip_proto())
            })
            .register_packet(packet);

        for p in result {
            match p {
                PacketType::Sent(p) => {
                    self.sent.push(p);
                }
                PacketType::Received(p) => {
                    self.received.push(p);
                }
            }
        }
    }

    pub fn get_latency_avg(&self) -> Option<f64> {
        self.sent.min_rtt()
    }

    pub fn periodic(&mut self) {
        self.streams.retain(|_, t| {
            t.last_registered.elapsed().unwrap() < crate::Settings::TCP_STREAM_TIMEOUT
        });
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
