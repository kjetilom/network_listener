use std::collections::HashMap;
use std::net::IpAddr;

use super::parser::ParsedPacket;
use super::procfs_reader::{NetEntry, NetStat};
use super::stream_id::ConnectionKey;
use super::tracker::{Tracker, TrackerState};


// Replace HashMap with DashMap
#[derive(Debug)]
pub struct StreamManager {
    // HashMap for all streams
    streams: HashMap<ConnectionKey, Tracker<TrackerState>>,
}


impl StreamManager {
    pub fn default() -> Self {
        StreamManager {
            streams: HashMap::new(),
        }
    }

    pub fn record_ip_packet(&mut self, packet: &ParsedPacket) {

        let stream_id = ConnectionKey::from_pcap(&packet);

        self.streams.entry(stream_id)
            .or_insert_with(|| Tracker::new(
                packet.timestamp,
                packet.transport.get_ip_proto()
            ))
            .register_packet(packet);
    }

    pub fn periodic(&mut self, proc_map: Option<NetStat>) {
        proc_map.map(|proc_map| self.update_states(proc_map));

        let seen_remote_ips: Vec<IpAddr> = self.streams.iter().map(|(k, _)| k.get_remote_ip()).collect();

        println!("Seen remote IPs: {:?}", seen_remote_ips);

        for (stream_id, tracker) in self.streams.iter() {
            match tracker.state {
                TrackerState::Tcp(ref tcp_tracker) => {
                    if let Some(bw) = tcp_tracker.stats.estimate_bandwidth() {
                        println!("Estimated bandwidth for {} : {:?} mb/s", stream_id, bw*8.0/1_000_000.0);
                    }
                }
                _ => {
                    println!("{}: Not a TCP stream", stream_id);
                }
            }
        }
    }

    fn update_states(&mut self, nstat: NetStat){

        let mut ids_to_remove: Vec<ConnectionKey> = Vec::new();

        for (stream_id, tracker) in self.streams.iter_mut() {
            match tracker.state {
                TrackerState::Tcp(ref mut tcp_tracker) => {
                    match nstat.tcp.get(stream_id) {
                        Some(NetEntry::Tcp { entry }) => {
                            tcp_tracker.stats.state = Some(entry.state.clone());
                        }
                        _ => {
                            ids_to_remove.push(*stream_id);
                        }
                    }
                }
                TrackerState::Udp(ref mut udp_tracker) => {
                    match nstat.udp.get(stream_id) {
                        Some(NetEntry::Udp { entry }) => {
                            udp_tracker.state = Some(entry.state.clone());
                        }
                        _ => {
                            ids_to_remove.push(*stream_id);
                        }
                    }
                }
                TrackerState::Other(ref mut _t) => {
                    if tracker.last_registered.elapsed().unwrap().as_secs() > 60 {
                        ids_to_remove.push(*stream_id);
                    }
                }
            }
        }
        self.streams.retain(|k, _| !ids_to_remove.contains(k));
    }

}
