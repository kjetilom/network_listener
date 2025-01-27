use std::{collections::HashMap, net::IpAddr};

use crate::listener::{packet::packet_builder::ParsedPacket, tracker::stream_manager::StreamManager};

type Streams = HashMap<IpPair, StreamManager>;

#[derive(Debug, Hash, Eq, PartialEq)]
struct IpPair {
    pair: (IpAddr, IpAddr),
}

impl IpPair {
    fn new(ip1: IpAddr, ip2: IpAddr) -> Self {
        IpPair {
            pair:
             if ip1 < ip2 {
                (ip1, ip2)
            } else {
                (ip2, ip1)
            },
        }
    }

    fn from_packet(packet: &ParsedPacket) -> Self {
        IpPair::new(packet.dst_ip, packet.src_ip)
    }

    fn get_pair(&self) -> (IpAddr, IpAddr) {
        self.pair
    }
}

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
        let ip_pair = IpPair::from_packet(&packet);
        self.links.entry(ip_pair)
            .or_insert_with(StreamManager::default)
            .record_ip_packet(&packet);
    }

    // pub fn periodic(&mut self, proc_map: Option<NetStat>) {
    //     for (_ip_pair, stream_manager) in self.links.iter_mut() {
    //         stream_manager.periodic(&proc_map);
    //     }
    // }
}