use std::collections::HashMap;
use std::net::IpAddr;

use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use procfs::net::UdpState;

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

pub enum Direction {
    Incoming,
    Outgoing,
}

impl Direction {
    pub fn from_packet(packet: &ParsedPacket, own_ip: IpAddr) -> Self {
        if packet.src_ip == own_ip {
            Direction::Outgoing
        } else {
            Direction::Incoming
        }
    }
}

impl StreamManager {
    pub fn default() -> Self {
        StreamManager {
            tcp_streams: HashMap::new(),
            udp_streams: HashMap::new(),
            other_streams: HashMap::new(),
        }
    }

    pub fn record_ip_packet(&mut self, packet: &ParsedPacket, own_ip: IpAddr) {
        let direction = Direction::from_packet(packet, own_ip);

        match packet.transport {
            TransportPacket::TCP { .. } => self.record_tcp(packet, own_ip),
            TransportPacket::UDP { .. } => self.record_udp(packet, own_ip),
            _ => self.record_other(packet, own_ip),
        };
    }

    fn record_tcp(&mut self, packet: &ParsedPacket, own_ip: IpAddr) {
        let stream_id = ConnectionKey::from_pcap(&packet, own_ip);

        let tracker = self.tcp_streams.entry(stream_id)
            .or_insert_with(|| Tracker::new(packet.timestamp, IpNextHeaderProtocols::Tcp));

        if tracker.last_registered != packet.timestamp {
            tracker.last_registered = packet.timestamp;
        }

        if packet.src_ip == own_ip {
            // Handle packets sent from own IP
            tracker.state.get_or_insert_with(|| TcpTracker::new())
                .handle_outgoing_packet(packet);
        } else {
            // Handle packets received by own IP
            tracker.state.get_or_insert_with(|| TcpTracker::new())
                .handle_incoming_packet(packet);
        }
    }

    fn record_udp(&mut self, packet: &ParsedPacket, own_ip: IpAddr) {
        let key = ConnectionKey::from_pcap(&packet, own_ip);

        let tracker = self.udp_streams.entry(key)
            .or_insert_with(|| Tracker::new(packet.timestamp, IpNextHeaderProtocols::Udp));

        if tracker.last_registered != packet.timestamp {
            tracker.last_registered = packet.timestamp;
        }

        // You may need to initialize the UdpTracker state here if needed
        tracker.state.get_or_insert_with(|| UdpTracker {state: Some(UdpState::Established)});
    }

    fn record_other(&mut self, packet: &ParsedPacket, _own_ip: IpAddr) {
        let key = ConnectionKey::from_pcap(&packet, _own_ip);
        if let TransportPacket::OTHER { protocol } = packet.transport {
            let tracker = self.other_streams.entry(key)
                .or_insert_with(|| Tracker::new(packet.timestamp, IpNextHeaderProtocol(protocol)));

            tracker.register_packet(packet);
        }
    }

    pub fn periodic(&mut self, proc_map: Option<NetStat>) {
        proc_map.map(|proc_map| self.update_states(proc_map));
    }

    fn update_states(&mut self, nstat: NetStat) {
        self.tcp_streams.retain(|stream_id, tracker| {
            match nstat.tcp.get(stream_id) {
                Some(NetEntry::Tcp { entry }) => {
                    tracker.state.as_mut().unwrap().stats.state = Some(entry.state.clone());
                    true
                },
                _ => false,
            }
        });

        self.udp_streams.retain(|stream_id, tracker| {
            match nstat.tcp.get(stream_id) {
                Some(NetEntry::Udp { entry }) => {
                    tracker.state.as_mut().unwrap().state = Some(entry.state.clone());
                    true
                },
                _ => false,
            }
        });
    }
}
