use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

use super::parser::{ParsedPacket, TransportPacket};
use super::procfs_reader::{NetEntry, NetStat};
use super::stream_id::StreamId;
use super::tracker::{TcpTracker, UdpTracker};


#[derive(Debug)]
pub struct StreamManager {
    tcp_streams: HashMap<StreamId, TcpTracker>,
    udp_streams: HashMap<StreamId, UdpTracker>,
}

impl StreamManager {
    pub fn new() -> Self {
        StreamManager {
            tcp_streams: HashMap::new(),
            udp_streams: HashMap::new(),
        }
    }

    pub fn record_packet(&mut self, packet: &ParsedPacket, own_ip: IpAddr) -> Option<Duration> {
        match packet.transport {
            TransportPacket::TCP { .. } => self.record_tcp(packet, own_ip),
            TransportPacket::UDP { .. } => self.record_udp(packet, own_ip),
            _ => None,
        }
    }

    fn record_tcp(&mut self, packet: &ParsedPacket, own_ip: IpAddr) -> Option<Duration> {
        let stream_id = StreamId::from_pcap(packet, own_ip);

        let tracker = self.tcp_streams.entry(stream_id)
            .or_insert_with(|| TcpTracker::new());

        tracker.last_registered = packet.timestamp;

        let is_syn = packet.transport.is_syn();
        let is_ack = packet.transport.is_ack();

        // !This needs to be fixed. It only supports one direction of communication.
        if packet.src_ip == own_ip {
            // Handle packets sent from own IP
            tracker.handle_outgoing_packet(packet, is_syn, is_ack);
            None
        } else {
            // Handle packets received by own IP
            tracker.handle_incoming_packet(packet, is_syn, is_ack)
        }
    }

    fn record_udp(&mut self, packet: &ParsedPacket, own_ip: IpAddr) -> Option<Duration> {
        let stream_id = StreamId::from_pcap(packet, own_ip);

        let tracker = self.udp_streams.entry(stream_id)
            .or_insert_with(|| UdpTracker {
                last_registered: packet.timestamp,
                state: None,
            });

        tracker.last_registered = packet.timestamp;
        None
    }

    pub fn periodic(&mut self, proc_map: Option<NetStat>) {
        match proc_map {
            Some(proc_map) => self.update_states(proc_map),
            None => (),
        };
    }

    fn update_states(&mut self, nstat: NetStat) {
        self.tcp_streams.retain(
            |stream_id, tracker| {
                if let Some(net_entry) = nstat.tcp.get(stream_id) {
                    match net_entry {
                        NetEntry::Tcp { entry } => {
                            tracker.state = Some(entry.state.clone());
                            true
                        },
                        _ => false,
                    }
                } else {
                    false
                }
            }
        );

        self.udp_streams.retain(
            |stream_id, tracker| {
                if let Some(net_entry) = nstat.udp.get(stream_id) {
                    match net_entry {
                        NetEntry::Udp { entry } => {
                            tracker.state = Some(entry.state.clone());
                            true
                        },
                        _ => false,
                    }
                } else {
                    false
                }
            }
        );
    }
}
