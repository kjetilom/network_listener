use std::collections::HashMap;

use super::parser::{ParsedPacket, TransportPacket};
use super::procfs_reader::{NetEntry, NetStat};
use super::stream_id::ConnectionKey;
use super::tracker::{GenericTracker, TcpTracker, Tracker, UdpTracker};


// Replace HashMap with DashMap
#[derive(Debug)]
pub struct StreamManager {
    tcp_streams: HashMap<ConnectionKey, Tracker<TcpTracker>>,
    udp_streams: HashMap<ConnectionKey, Tracker<UdpTracker>>,
    other_streams: HashMap<ConnectionKey, Tracker<GenericTracker>>,
}


impl StreamManager {
    pub fn default() -> Self {
        StreamManager {
            tcp_streams: HashMap::new(),
            udp_streams: HashMap::new(),
            other_streams: HashMap::new(),
        }
    }

    pub fn record_ip_packet(&mut self, packet: &ParsedPacket) {

        let stream_id = ConnectionKey::from_pcap(&packet);

        match packet.transport {
            TransportPacket::TCP { .. } => {
                self.tcp_streams.entry(stream_id)
                    .or_insert_with(|| Tracker::new(
                        packet.timestamp,
                        packet.transport.get_ip_proto()
                    ))
                    .register_packet(packet);
            }
            TransportPacket::UDP { .. } => {
                self.udp_streams.entry(stream_id)
                    .or_insert_with(|| Tracker::new(
                        packet.timestamp,
                        packet.transport.get_ip_proto()
                    ))
                    .register_packet(packet);
            }
            _ => {
                self.other_streams.entry(stream_id)
                    .or_insert_with(|| Tracker::new(
                        packet.timestamp,
                        packet.transport.get_ip_proto()
                    ))
                    .register_packet(packet);
            }
        }
    }

    pub fn periodic(&mut self, proc_map: Option<NetStat>) {
        proc_map.map(|proc_map| self.update_states(proc_map));
    }

    fn update_states(&mut self, nstat: NetStat) {
        self.tcp_streams.retain(|stream_id, tracker| {
            match nstat.tcp.get(stream_id) {
                Some(NetEntry::Tcp { entry }) => {
                    tracker.state.stats.state = Some(entry.state.clone());
                    true
                },
                _ => false,
            }
        });

        self.udp_streams.retain(|stream_id, tracker| {
            match nstat.tcp.get(stream_id) {
                Some(NetEntry::Udp { entry }) => {
                    tracker.state.state = Some(entry.state.clone());
                    true
                },
                _ => false,
            }
        });
        self.other_streams.retain(|_, tracker| {
            tracker.last_registered.elapsed().unwrap().as_secs() < 60
        }

        );
    }
}
