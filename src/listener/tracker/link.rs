use std::collections::HashMap;

use crate::listener::{packet::packet_builder::ParsedPacket, tracker::stream_manager::StreamManager};

use super::stream_id::IpPair;

type Streams = HashMap<IpPair, StreamManager>;

#[derive(Debug)]
pub struct LinkManager {
    links: Streams, // Private field
}

impl LinkManager {
    pub fn new() -> Self {
        LinkManager {
            links: HashMap::new(),
        }
    }

    pub fn insert(&mut self, packet: ParsedPacket) {
        // Ignore if loopback
        if packet.src_ip.is_loopback() || packet.dst_ip.is_loopback() {
            return;
        }
        let ip_pair = IpPair::from_packet(&packet);

        self.links.entry(ip_pair)
            .or_insert_with(StreamManager::default)
            .record_ip_packet(&packet);
    }

    pub fn periodic(&mut self) {
        for (ip_pair, stream_manager) in self.links.iter_mut() {
            if stream_manager.get_latency_avg().is_none() {
                continue;
            }
            println!("{} {}", ip_pair, stream_manager.get_latency_avg().unwrap_or(0.0));
        }
    }
}