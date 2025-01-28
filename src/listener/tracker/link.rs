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
        println!();
        for (ip_pair, stream_manager) in self.links.iter_mut() {
            let data_in_out = stream_manager.get_in_out();
            let latency = stream_manager.get_latency_avg();
            let rt_in_out = stream_manager.get_rt_in_out();
            let in_ = data_in_out.0 as f64 / 1024.0 / 1.0; // INSERT THING HERE
            let out = data_in_out.1 as f64 / 1024.0 / 1.0; // INSERT THING HERE
            if let Some(latency) = latency {
                println!(
                    "Link: {} - In: {:.2} KB/s, Out: {:.2} KB/s, Latency: {:.2} ms, rts in({}) out({})",
                    ip_pair, in_, out, latency*1000.0, rt_in_out.0, rt_in_out.1
                );
            }
            stream_manager.periodic();
        }
    }
}