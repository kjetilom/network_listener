use crate::{
    stream_id::StreamKey,
    tracker::{Tracker, TrackerState},
    PacketRegistry, ParsedPacket,
};
use pnet::packet::ip::IpNextHeaderProtocol;
use std::{collections::HashMap, time::SystemTime};
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
    bytes_sent: u32,
    bytes_received: u32,
}

impl StreamManager {
    pub fn default() -> Self {
        StreamManager {
            streams: HashMap::new(),
            sent: PacketRegistry::new(),
            received: PacketRegistry::new(),
            tcp_thput: 0.0,
            last_iperf: None,
            bytes_sent: 0,
            bytes_received: 0,
        }
    }

    pub fn record_iperf_result(&mut self, bps: f64, _stream: Option<&crate::IperfStream>) {
        // Check if in out is very different
        self.last_iperf = Some(Instant::now());
        self.tcp_thput = bps;
    }

    pub fn drain_rtts(&mut self) -> Vec<(u32, SystemTime)> {
        self.sent.take_rtts()
    }

    pub fn tcp_thput(&self) -> f64 {
        if let Some(last_iperf) = self.last_iperf {
            if last_iperf.elapsed() > crate::CONFIG.client.measurement_window {
                return self.tcp_thput
            }
        }
        return 0.0
    }

    pub fn abw(&mut self) -> Option<f64> {
        self.sent.passive_pgm_abw_rls()
    }

    pub fn record_packet(&mut self, packet: &ParsedPacket) {

        match packet.direction {
            crate::Direction::Incoming => {
                self.bytes_received += packet.total_length as u32;
            }
            crate::Direction::Outgoing => {
                self.bytes_sent += packet.total_length as u32;
            }
        }

        let stream_id = StreamKey::from_packet(packet);
        let (burst, direction) = match self
            .streams
            .entry(stream_id)
            .or_insert_with(|| {
                Tracker::<TrackerState>::new(packet.timestamp, packet.transport.get_ip_proto())
            })
            .register_packet(packet)
        {
            Some((burst, direction)) => (burst, direction),
            None => return,
        };

        match direction {
            crate::Direction::Incoming => {
                self.received.extend(burst);
            }
            crate::Direction::Outgoing => {
                self.sent.extend(burst);
            }
        }
    }

    pub fn get_latency_avg(&self) -> Option<f64> {
        self.sent.avg_rtt()
    }

    pub fn get_sent(&mut self) -> u32 {
        std::mem::take(&mut self.bytes_sent)
    }

    pub fn get_received(&mut self) -> u32 {
        std::mem::take(&mut self.bytes_received)
    }

    pub fn periodic(&mut self) {
        for stream in self.streams.values_mut() {
            let (sent, received) = match stream.state {
                TrackerState::Tcp(ref mut tracker) => {
                    tracker.take_bursts()
                }
                TrackerState::Udp(ref mut tracker) => {
                    tracker.take_bursts()
                }
                TrackerState::Other(ref mut tracker) => {
                    tracker.take_bursts()
                }
            };
            self.sent.extend(sent);
            self.received.extend(received);
        }
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

    pub fn get_streams(&self, protocol: IpNextHeaderProtocol) -> Vec<&Tracker<TrackerState>> {
        self.streams
            .values()
            .filter(|t| t.protocol == protocol)
            .collect()
    }
}
