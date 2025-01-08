use std::net::IpAddr;
use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use std::collections::HashMap;

use super::packet::packet_builder::ParsedPacket;
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
                        println!("Estimated bandwidth for {} : {:?} Mb/s", stream_id, bw*8.0/1_000_000.0);
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
            if tracker.last_registered.elapsed().unwrap().as_secs() > 60 {
                ids_to_remove.push(*stream_id);
                continue;
            }
            match tracker.state {
                TrackerState::Tcp(ref mut tcp_tracker) => {
                    match nstat.tcp.get(stream_id) {
                        Some(NetEntry::Tcp { entry }) => {
                            tcp_tracker.stats.state = Some(entry.state.clone());
                        }
                        _ => {}
                    }
                    if matches!(tcp_tracker.stats.state, Some(procfs::net::TcpState::Close)) {
                        dbg!(&tcp_tracker.stats.state);
                        ids_to_remove.push(*stream_id)
                    }
                }
                TrackerState::Udp(ref mut udp_tracker) => {
                    match nstat.udp.get(stream_id) {
                        Some(NetEntry::Udp { entry }) => {
                            udp_tracker.state = Some(entry.state.clone());
                        }
                        _ => {}
                    }
                    if matches!(udp_tracker.state, Some(procfs::net::UdpState::Close)) {
                        ids_to_remove.push(*stream_id)
                    }
                }
                _ => {}
            }
        }
        self.streams.retain(|k, _| !ids_to_remove.contains(k));

    }

    pub fn netstat_diff(&self, nstat: NetStat) -> Vec<ConnectionKey> {
        let mut diff = Vec::new();

        for (stream_id, _tracker) in self.streams.iter() {
            if !nstat.tcp.contains_key(stream_id) && !nstat.udp.contains_key(stream_id) {
                diff.push(*stream_id);
            }
        }
        diff
    }

    pub fn take_streams(&mut self, keys: Vec<ConnectionKey>) -> Vec<Tracker<TrackerState>> {
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
        self.streams.values().filter(|t| t.protocol == protocol).collect()
    }

    /// Take all streams of a given protocol from the manager
    pub fn take_streams_by_protocol(&mut self, protocol: IpNextHeaderProtocol) -> Vec<Tracker<TrackerState>> {
        let mut taken = Vec::new();

        // Use `drain` to take ownership of the entries in the HashMap
        self.streams = self.streams.drain().filter_map(|(key, value)| {
            if value.protocol == protocol {
                taken.push(value);
                None
            } else {
                Some((key, value))
            }
        }).collect();

        taken
    }

    pub fn get_key_by_protocol(&self, protocol: IpNextHeaderProtocol) -> Vec<ConnectionKey> {
        self.streams.keys().filter(|k| k.get_protocol() == protocol).cloned().collect()
    }
}
