use std::collections::BTreeMap;
use std::time::SystemTime;

use tokio::time::Duration;

use crate::{Direction, PacketType, ParsedPacket, TransportPacket};

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
pub struct TcpBurst {
    pub packets: Vec<Acked>,
}

pub enum Burst {
    Tcp(TcpBurst),
    Udp(Vec<PacketType>),
    Other(Vec<PacketType>),
}

impl Default for TcpBurst {
    fn default() -> Self {
        TcpBurst {
            packets: Vec::new(),
        }
    }
}

impl Burst {
    pub fn flatten(self) -> Vec<PacketType> {
        match self {
            Burst::Tcp(burst) => burst.flatten(),
            Burst::Udp(packets) => packets,
            Burst::Other(packets) => packets,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Burst::Tcp(burst) => burst.packets.is_empty(),
            Burst::Udp(packets) => packets.is_empty(),
            Burst::Other(packets) => packets.is_empty(),
        }
    }

    fn get_time_duration(packets: &Vec<PacketType>) -> Option<Duration> {
        if packets.len() > 1 {
            let mut first = SystemTime::UNIX_EPOCH;
            let mut last = SystemTime::UNIX_EPOCH;
            for packet in packets {
                if packet.sent_time < first {
                    first = packet.sent_time;
                }
                if packet.sent_time > last {
                    last = packet.sent_time;
                }
            }
            match last.duration_since(first) {
                Ok(d) => return Some(d),
                Err(_) => return None,
            }
        }
        None
    }

    fn get_throughput(packets: &Vec<PacketType>) -> f64 {
        if let Some(d) = Self::get_time_duration(packets) {
            packets.iter().map(|p| p.total_length as f64).sum::<f64>() / d.as_secs_f64()
        } else {
            0.0
        }
    }

    pub fn burst_size_bytes(&self) -> u64 {
        match self {
            Burst::Tcp(burst) => burst.total_length() as u64,
            Burst::Udp(packets) => packets.iter().map(|p| p.total_length as u64).sum(),
            Burst::Other(packets) => packets.iter().map(|p| p.total_length as u64).sum(),
        }
    }

    pub fn throughput(&self) -> f64 {
        match self {
            Burst::Tcp(burst) => burst.throughput().unwrap_or(0.0),
            Burst::Udp(packets) => Self::get_throughput(packets),
            Burst::Other(packets) => Self::get_throughput(packets),
        }
    }
}

impl TcpBurst {
    pub fn flatten(self) -> Vec<PacketType> {
        self.packets
            .into_iter()
            .flat_map(|acked| acked.acked_packets)
            .collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &PacketType> {
        self.packets.iter().flat_map(|acked| acked.iter())
    }

    pub fn total_length(&self) -> u32 {
        self.packets.iter().map(|acked| acked.total_length).sum()
    }

    pub fn time_duration(&self) -> Option<Duration> {
        if let Some(first) = self.packets.first() {
            let first = first.acked_packets.first().unwrap().sent_time;
            let last = self.packets.last().unwrap().ack_time;
            match last.duration_since(first) {
                Ok(d) => Some(d),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    pub fn throughput(&self) -> Option<f64> {
        if let Some(d) = self.time_duration() {
            Some(self.total_length() as f64 / d.as_secs_f64())
        } else {
            None
        }
    }
}

impl From<TcpBurst> for Burst {
    fn from(burst: TcpBurst) -> Self {
        Burst::Tcp(burst)
    }
}

#[derive(Debug)]
pub struct Acked {
    acked_packets: Vec<PacketType>,
    pub ack_time: SystemTime,
    first_sent_time: Option<SystemTime>,
    last_sent_time: SystemTime,
    pub total_length: u32,
}

impl Acked {
    fn from_acked(acked_packets: Vec<PacketType>, ack_time: SystemTime, first_sent_time: Option<SystemTime>) -> Self {
        let last_sent_time = acked_packets.last().unwrap().sent_time;
        let total_length = acked_packets.iter().map(|p| p.total_length as u32).sum();
        Acked {
            acked_packets,
            ack_time,
            first_sent_time,
            last_sent_time,
            total_length,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &PacketType> {
        self.acked_packets.iter()
    }

    pub fn get_gin_gout_len(&self, last_ack: SystemTime) -> Option<(f64, f64, u32)> {
        if let Some(first_sent_time) = self.first_sent_time {
            let gin = self
                .last_sent_time
                .duration_since(first_sent_time)
                .ok()?;
            let gout = self.ack_time.duration_since(last_ack).ok()?;
            let total_length = self
                .acked_packets
                .iter()
                .map(|p| p.payload_len as u32)
                .sum::<u32>();
            Some((
                gin.as_secs_f64(),
                gout.as_secs_f64(),
                total_length,
            ))
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.acked_packets.len()
    }
}

#[derive(Debug)]
struct TcpStream {
    packets: BTreeMap<u32, PacketType>,
    last_ack: Option<SystemTime>,
    last_sent: Option<SystemTime>,
    last_registered: Option<SystemTime>,
    cur_burst: TcpBurst,
    max_rtt: Duration,
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
        let gap: Option<Duration> = match self.last_ack {
            Some(last_ack) => match new.duration_since(last_ack) {
                Ok(d) => Some(d),
                Err(_) => None,
            },
            None => None,
        };
        self.last_ack = Some(new);
        gap
    }

    fn register_packet(&mut self, packet: &ParsedPacket) -> Option<TcpBurst> {
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
            if self.cur_burst.packets.len() > 0 {
                if let Some(last_registered) = self.last_registered {
                    if let Ok(d) = packet.timestamp.duration_since(last_registered) {
                        if d > self.max_rtt || self.cur_burst.packets.len() > 100 {
                            // Indiana Jones moment (Replace self.cur_burst with default)
                            ret = Some(std::mem::take(&mut self.cur_burst));
                            self.last_registered = None;
                            self.max_rtt = self.max_rtt / 2;
                        }
                    }
                }
            }

            if flags.is_ack() && *payload_len == 0 {
                // Pure ACK acknowledges local packets.
                pkt.gap_last_ack = self.get_gap_last_ack(pkt.sent_time);
                acked_packets = self.update_acked_packets(*acknowledgment, pkt);
            } else {
                // Set new last sent time and calculate gap
                pkt.gap_last_sent = self.get_gap_last_sent(pkt.sent_time);
                self.track_packet(*sequence, pkt);
            }
        }
        if acked_packets.len() > 0 {
            let last_sent: Option<SystemTime> = if let Some(prev_ack) = self.cur_burst.packets.last() {
                Some(prev_ack.last_sent_time)
            } else {
                None
            };
            self.cur_burst
                .packets
                .push(Acked::from_acked(acked_packets, packet.timestamp, last_sent));
        }
        self.last_registered = Some(packet.timestamp);
        ret
    }

    fn track_packet(&mut self, sequence: u32, packet: PacketType) {
        match self.packets.get_mut(&sequence) {
            Some(existing) => {
                existing.retransmissions += 1;
                // If we don't do this we will calculate a way too high RTT
                existing.sent_time = packet.sent_time;
                existing.gap_last_sent = packet.gap_last_sent;
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
        let mut bytes_acked = None;
        for (&seq, sent_packet) in self.packets.iter_mut() {
            if seq_less_equal(seq.wrapping_add(sent_packet.payload_len as u32), ack) {
                // Set bytes acked to the first packet that is acked (this works due to the map being sorted)
                if bytes_acked.is_none() {
                    bytes_acked = Some(seq.wrapping_sub(ack));
                }

                if let Ok(rtt_duration) = pkt.sent_time.duration_since(sent_packet.sent_time) {
                    self.max_rtt = std::cmp::max(self.max_rtt, rtt_duration);
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
                last_ack: None,
                last_sent: None,
                last_registered: None,
                cur_burst: TcpBurst::default(),
                max_rtt: Duration::from_secs(10),
            },
            received: TcpStream {
                packets: BTreeMap::new(),
                last_ack: None,
                last_sent: None,
                last_registered: None,
                cur_burst: TcpBurst::default(),
                max_rtt: Duration::from_secs(10),
            },
        }
    }

    pub fn take_bursts(&mut self) -> (Burst, Burst) {
        let sent = std::mem::take(&mut self.sent.cur_burst);
        let received = std::mem::take(&mut self.received.cur_burst);
        (sent.into(), received.into())
    }

    pub fn register_packet(&mut self, packet: &ParsedPacket) -> Option<(Burst, Direction)> {
        let (burst, direction) = match packet.direction {
            Direction::Incoming => {
                if packet.is_pure_ack() {
                    (self.sent.register_packet(packet), Direction::Outgoing)
                } else {
                    (self.received.register_packet(packet), Direction::Incoming)
                }
            }
            Direction::Outgoing => {
                if packet.is_pure_ack() {
                    (self.received.register_packet(packet), Direction::Incoming)
                } else {
                    (self.sent.register_packet(packet), Direction::Outgoing)
                }
            }
        };
        if let Some(burst) = burst {
            Some((burst.into(), direction))
        } else {
            None
        }
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
    }
}
