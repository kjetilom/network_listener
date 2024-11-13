use std::collections::HashMap;
use std::net::IpAddr;

use log::info;

use super::parser::{ParsedPacket, TransportPacket};
use super::procfs_reader::{NetEntry, NetStat};
use super::stream_id::StreamId;
use super::tracker::{TcpTracker, UdpTracker};


// Replace HashMap with DashMap
#[derive(Debug)]
pub struct StreamManager {
    tcp_streams: HashMap<StreamId, TcpTracker>,
    udp_streams: HashMap<StreamId, UdpTracker>,
}

impl StreamManager {
    pub fn default() -> Self {
        StreamManager {
            tcp_streams: HashMap::new(),
            udp_streams: HashMap::new(),
        }
    }

    pub fn record_ip_packet(&mut self, packet: ParsedPacket, own_ip: IpAddr) {

        match packet.transport {
            TransportPacket::TCP { .. } => self.record_tcp(packet, own_ip),
            TransportPacket::UDP { .. } => self.record_udp(packet, own_ip),
            TransportPacket::ICMP { .. } => self.record_icmp(packet, own_ip),
            TransportPacket::OTHER { .. } => self.record_other(packet, own_ip),
        };
    }

    fn record_tcp(&mut self, packet: ParsedPacket, own_ip: IpAddr) {
        let stream_id = StreamId::from_pcap(&packet, own_ip);

        let tracker = self.tcp_streams.entry(stream_id)
            .or_insert_with(|| TcpTracker::new(packet.timestamp));

        if tracker.last_registered != packet.timestamp {
            tracker.last_registered = packet.timestamp;
        }

        if packet.src_ip == own_ip {
            // Handle packets sent from own IP
            tracker.handle_outgoing_packet(packet);
        } else {
            // Handle packets received by own IP
            tracker.handle_incoming_packet(packet);
        }
    }

    fn record_udp(&mut self, packet: ParsedPacket, own_ip: IpAddr) {
        let stream_id = StreamId::from_pcap(&packet, own_ip);

        let tracker = self.udp_streams.entry(stream_id)
            .or_insert_with(|| UdpTracker {
                last_registered: packet.timestamp,
                state: None,
            });

        if tracker.last_registered != packet.timestamp {
            tracker.last_registered = packet.timestamp;
        }
    }

    fn record_icmp(&mut self, packet: ParsedPacket, _own_ip: IpAddr) {
        info!("ICMP packet received: {:?}", packet);
    }

    fn record_other(&mut self, packet: ParsedPacket, _own_ip: IpAddr) {
        if let TransportPacket::OTHER { protocol, .. } = packet.transport {
            log::info!("{} packet received", pnet::packet::ip::IpNextHeaderProtocol(protocol));
        }
    }

    pub fn periodic(&mut self, proc_map: Option<NetStat>) {
        proc_map.map(|proc_map| self.update_states(proc_map));
    }

    fn update_states(&mut self, nstat: NetStat) {
        self.tcp_streams.retain(|stream_id, tracker| {
            match nstat.tcp.get(stream_id) {
                Some(NetEntry::Tcp { entry }) => {
                    tracker.stats.state = Some(entry.state.clone());
                    true
                },
                _ => false,
            }
        });

        self.udp_streams.retain(|stream_id, tracker| {
            match nstat.tcp.get(stream_id) {
                Some(NetEntry::Udp { entry }) => {
                    tracker.state = Some(entry.state.clone());
                    true
                },
                _ => false,
            }
        });
    }
}
