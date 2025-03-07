use std::collections::BTreeMap;
use std::time::SystemTime;

use tokio::time::Duration;
use pnet::packet::ip::IpNextHeaderProtocol;

use crate::{
    tracker::DefaultState, Direction, PacketType, ParsedPacket, TransportPacket
};

/// Wrap-around aware sequence comparison.
fn seq_cmp(a: u32, b: u32) -> i32 {
    a.wrapping_sub(b) as i32
}
fn seq_less_equal(a: u32, b: u32) -> bool {
    seq_cmp(a, b) <= 0
}

/// A burst of packets.
/// Is stored before being returned to the packet_registry for processing
#[derive(Debug)]
pub struct Burst {
    packets: Vec<Acked>,
}

impl Default for Burst {
    fn default() -> Self {
        Burst {
            packets: Vec::new(),
        }
    }
}

impl Burst {
    fn flatten(self) -> Vec<PacketType> {
        self.packets
            .into_iter()
            .flat_map(|acked| acked.acked_packets)
            .collect()
    }
}

#[derive(Debug)]
pub struct Acked {
    acked_packets: Vec<PacketType>,
    ack_time: SystemTime,
    first_sent_time: SystemTime,
    last_sent_time: SystemTime,
}

impl Acked {
    fn from_acked(acked_packets: Vec<PacketType>, ack_time: SystemTime) -> Self {
        let first_sent_time = acked_packets.first().unwrap().sent_time;
        let last_sent_time = acked_packets.last().unwrap().sent_time;
        Acked {
            acked_packets,
            ack_time,
            first_sent_time,
            last_sent_time,
        }
    }
}

#[derive(Debug)]
struct TcpStream {
    packets: BTreeMap<u32, PacketType>,
    last_ack: SystemTime,
    last_sent: Option<SystemTime>,
    cur_burst: Burst,
    min_rtt: Duration,
}

impl TcpStream {
    fn get_gap_last_sent(&mut self, new: SystemTime) -> Option<Duration> {
        let gap: Option<Duration> = match self.last_sent {
            Some(last_sent) => match new.duration_since(last_sent) {
                Ok(d) => Some(d),
                Err(_) => None,
            },
            None => None,
        };
        self.last_sent = Some(new);
        gap
    }

    fn get_gap_last_ack(&mut self, new: SystemTime) -> Option<Duration> {
        let gap: Option<Duration> = match self.last_ack.duration_since(new) {
            Ok(d) => Some(d),
            Err(_) => None,
        };
        self.last_ack = new;
        gap
    }

    fn register_packet(&mut self, packet: &ParsedPacket) -> Option<Burst> {
        let mut acked_packets = Vec::new();
        let mut ret = None;

        if let TransportPacket::TCP {
            sequence,
            acknowledgment,
            payload_len,
            flags,
            ..
        } = &packet.transport
        {
            let mut pkt = PacketType::from_packet(packet);
            if let Some(last_sent) = self.last_sent {
                if let Ok(d) = packet.timestamp.duration_since(last_sent) {
                    if d > self.min_rtt * 4 {
                        // Indiana Jones moment (Replace self.cur_burst with default)
                        ret = Some(std::mem::take(&mut self.cur_burst));
                    }
                }
            }

            if flags.is_ack() && *payload_len == 0 {
                // Pure ACK acknowledges local packets.
                pkt.gap_last_ack = self.get_gap_last_ack(packet.timestamp);
                acked_packets = self.update_acked_packets(*acknowledgment, pkt);
            } else {
                // Set new last sent time and calculate gap
                pkt.gap_last_sent = self.get_gap_last_sent(packet.timestamp);
                self.track_packet(*sequence, pkt);
            }
        }
        if acked_packets.len() > 0 {
            self.cur_burst.packets.push(Acked::from_acked(acked_packets, packet.timestamp));
        }
        ret
    }

    fn track_packet(&mut self, sequence: u32, packet: PacketType) {
        match self.packets.get_mut(&sequence) {
            Some(existing) => {
                existing.retransmissions += 1;
                // If we don't do this we will calculate a way too high RTT
                existing.sent_time = packet.sent_time;
            }
            None => {
                self.packets.insert(sequence, packet);
            }
        }
    }

    /// Update and remove all packets in the provided map that are
    /// fully acknowledged. Also update RTT and register the "sent" packet.
    fn update_acked_packets(&mut self, ack: u32, pkt: PacketType) -> Vec<PacketType> {
        let mut acked = Vec::new();
        let mut keys_to_remove = Vec::new();
        for (&seq, sent_packet) in self.packets.iter_mut() {
            if seq_less_equal(seq.wrapping_add(sent_packet.payload_len as u32), ack) {
                if let Ok(rtt_duration) = pkt.sent_time.duration_since(sent_packet.sent_time) {
                    self.min_rtt = std::cmp::min(self.min_rtt, rtt_duration);
                    sent_packet.rtt = Some(rtt_duration);
                    sent_packet.ack_time = Some(pkt.sent_time);
                    sent_packet.gap_last_ack = pkt.gap_last_ack;
                }
                keys_to_remove.push(seq);
            } else {
                break;
            }
        }

        for seq in keys_to_remove {
            if let Some(p) = self.packets.remove(&seq) {
                acked.push(p);
            }
        }

        acked.sort_by(|a, b| a.sent_time.cmp(&b.sent_time));
        acked
    }
}

/// TCP tracker which now tracks packets from both directions.
/// Outgoing packets (from us) are stored in `local_sent_packets` and
/// packets from the remote side are stored in `remote_sent_packets`.
#[derive(Debug)]
pub struct TcpTracker {
    sent: TcpStream,
    received: TcpStream,
}

impl Default for TcpTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpTracker {
    pub fn new() -> Self {
        TcpTracker {
            sent: TcpStream {
                packets: BTreeMap::new(),
                last_ack: SystemTime::UNIX_EPOCH,
                last_sent: None,
                cur_burst: Burst::default(),
                min_rtt: Duration::from_secs(10),
            },
            received: TcpStream {
                packets: BTreeMap::new(),
                last_ack: SystemTime::UNIX_EPOCH,
                last_sent: None,
                cur_burst: Burst::default(),
                min_rtt: Duration::from_secs(10),
            },
        }
    }

    pub fn register_packet(&mut self, packet: &ParsedPacket) -> Vec<PacketType> {
        let burst = match packet.direction {
            Direction::Incoming => {
                if packet.is_pure_ack() {
                    self.sent.register_packet(packet)
                } else {
                    self.received.register_packet(packet)
                }
            }
            Direction::Outgoing => {
                if packet.is_pure_ack() {
                    self.received.register_packet(packet)
                } else {
                    self.sent.register_packet(packet)
                }
            }
        };
        if let Some(burst) = burst {
            burst.flatten()
        } else {
            Vec::new()
        }
    }
}

impl DefaultState for TcpTracker {
    fn default(_protocol: IpNextHeaderProtocol) -> Self {
        Self::new()
    }
    fn register_packet(&mut self, packet: &ParsedPacket) -> Vec<PacketType> {
        self.register_packet(packet)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_mem_swap() {
        let mut b = vec![1, 2, 3];
        let a = std::mem::take(&mut b);

        assert_eq!(a, vec![1, 2, 3]);
        assert_eq!(b, Vec::<i32>::new());
    }

    #[test]
    fn test_sort_by_time() {
        use std::time::{Duration, SystemTime};

        #[derive(Debug, PartialEq)]
        struct Packet {
            sent_time: SystemTime,
        }

        let p1 = Packet {
            sent_time: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        };
        let p2 = Packet {
            sent_time: SystemTime::UNIX_EPOCH + Duration::from_secs(2),
        };
        let p3 = Packet {
            sent_time: SystemTime::UNIX_EPOCH + Duration::from_secs(3),
        };

        let mut packets = vec![p3, p1, p2];

        packets.sort_by(|a, b| a.sent_time.cmp(&b.sent_time));

        assert!(packets[0].sent_time < packets[1].sent_time);

        println!("{:?}", packets);
    }
}
