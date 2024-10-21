use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use super::parser::{ParsedPacket, TransportPacket};
use super::procfs_reader::netstat_test;
use super::stream_id::TcpStreamId;
use super::tracker::PacketTracker;


#[derive(Debug)]
pub struct TcpStreamManager {
    streams: HashMap<TcpStreamId, PacketTracker>,
    timeout: Duration,
    last_cleanup: Instant,
}

impl TcpStreamManager {
    pub fn new(timeout: Duration) -> Self {
        TcpStreamManager {
            streams: HashMap::new(),
            timeout,
            last_cleanup: Instant::now(),
        }
    }

    pub fn record_packet(&mut self, packet: &ParsedPacket, own_ip: IpAddr) -> Option<Duration> {
        if let TransportPacket::TCP { flags, .. } = &packet.transport {

            if self.last_cleanup.elapsed() > super::Settings::CLEANUP_INTERVAL {
                let inst = Instant::now();
                let proc_map = netstat_test();
                self.streams.retain(
                    |stream_id, tracker| {
                        if let Some((state, _, _, _)) = proc_map.get(stream_id) {
                            tracker.state = Some(state.clone());
                            true
                        } else {
                            false
                        }
                    }
                );
                println!("Cleanup took {:?}", inst.elapsed());
                for (stream_id, tracker) in self.streams.iter() {
                    println!("{}, State: {:?}, Elapsed {:?}", stream_id, tracker.state, tracker.last_registered.elapsed());
                    println!("Total retransmissions: {:?}", tracker.total_retransmissions);
                }

                self.last_cleanup = Instant::now();
            }

            let stream_id = TcpStreamId::from_pcap(packet, own_ip);

            let tracker = self.streams.entry(stream_id)
                .or_insert_with(|| PacketTracker::new(self.timeout));

            tracker.last_registered = packet.timestamp;

            let is_syn = flags & 0x02 != 0;
            let is_ack = flags & 0x10 != 0;

            if packet.src_ip == own_ip {
                // Handle packets sent from own IP
                tracker.handle_outgoing_packet(packet, is_syn, is_ack);
            } else {
                // Handle packets received by own IP
                return tracker.handle_incoming_packet(packet, is_syn, is_ack);
            }
        }
        None
    }

    /// Cleans up all streams by removing probes that have timed out.
    pub fn cleanup(&mut self) {

    }
}
