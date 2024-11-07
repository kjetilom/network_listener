use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

use super::parser::{ParsedPacket, TransportPacket};
use super::stream_id::StreamId;
use super::tracker::{TcpTracker, UdpTracker};


#[derive(Debug)]
pub struct StreamManager {
    tcp_streams: HashMap<StreamId, TcpTracker>,
    udp_streams: HashMap<StreamId, UdpTracker>,
    timeout: Duration,
}

impl StreamManager {
    pub fn new(timeout: Duration) -> Self {
        StreamManager {
            tcp_streams: HashMap::new(),
            udp_streams: HashMap::new(),
            timeout,
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
            .or_insert_with(|| TcpTracker::new(self.timeout));

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
            });

        tracker.last_registered = packet.timestamp;
        None
    }

    pub fn periodic(&mut self, proc_map: Option<HashMap<StreamId, (procfs::net::TcpState, u32, u32, u64)>>) {
        match proc_map {
            Some(proc_map) => self.update_states(proc_map),
            None => (),
        };

        for (stream_id, tracker) in self.tcp_streams.iter() {
            println!("{}, State: {:?}, Elapsed {:?}", stream_id, tracker.state, tracker.last_registered.elapsed());
        }

        for (stream_id, tracker) in self.tcp_streams.iter() {
            for (size, rtt) in tracker.rtt_to_size.iter() {
                println!("{}, RTT: {:?}, Size: {}", stream_id, rtt, size);
            }
        }

        // for (stream_id, tracker) in self.udp_streams.iter() {
        //     println!("{}, Elapsed {:?}", stream_id, tracker.last_registered.elapsed());
        // }
    }

    fn update_states(&mut self, proc_map: HashMap<StreamId, (procfs::net::TcpState, u32, u32, u64)>) {
        self.tcp_streams.retain(
            |stream_id, tracker| {
                if let Some((state, _, _, _)) = proc_map.get(stream_id) {
                    tracker.state = Some(state.clone());
                    true
                } else {
                    false
                }
            }
        );
    }

    /// Cleans up all streams by removing probes that have timed out.
    pub fn cleanup(&mut self) {

    }
}
