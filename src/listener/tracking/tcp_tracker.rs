use std::collections::BTreeMap;
use std::time::SystemTime;

use tokio::time::Duration;

use crate::{Direction, PacketType, ParsedPacket, TransportPacket};

/// Compare two TCP sequence numbers, taking into account wrap-around.
///
/// Returns a signed 32-bit difference: positive if `a` is ahead of `b`, negative if behind.
fn seq_cmp(a: u32, b: u32) -> i32 {
    a.wrapping_sub(b) as i32
}
fn seq_less_equal(a: u32, b: u32) -> bool {
    seq_cmp(a, b) <= 0
}

/// A burst of TCP packets that have been acknowledged together.
#[derive(Debug)]
pub struct TcpBurst {
    /// The list of acknowledge packets groups in order.
    pub packets: Vec<Acked>,
}

/// A generic packet burst, for TCP, UDP, or other protocols.
pub enum Burst {
    /// TCP burst with retransmission and RTT tracking.
    Tcp(TcpBurst),
    /// Simple UDP burst: a flat list of packets.
    Udp(Vec<PacketType>),
    /// Burst of other protocols.
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
    /// Consume the burst and return a flat `Vec<PacketType>`.
    pub fn flatten(self) -> Vec<PacketType> {
        match self {
            Burst::Tcp(burst) => burst.flatten(),
            Burst::Udp(packets) => packets,
            Burst::Other(packets) => packets,
        }
    }

    /// Returns `true` if the burst contains no packets.
    pub fn is_empty(&self) -> bool {
        match self {
            Burst::Tcp(burst) => burst.packets.is_empty(),
            Burst::Udp(packets) => packets.is_empty(),
            Burst::Other(packets) => packets.is_empty(),
        }
    }

    /// Helper to compute duration between first and last packet times.
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

    /// Compute throughput in bytes per second over the burst.
    fn get_throughput(packets: &Vec<PacketType>) -> f64 {
        if let Some(d) = Self::get_time_duration(packets) {
            packets.iter().map(|p| p.total_length as f64).sum::<f64>() / d.as_secs_f64()
        } else {
            0.0
        }
    }

    /// Total byte size of the burst.
    pub fn burst_size_bytes(&self) -> u64 {
        match self {
            Burst::Tcp(burst) => burst.total_length() as u64,
            Burst::Udp(packets) => packets.iter().map(|p| p.total_length as u64).sum(),
            Burst::Other(packets) => packets.iter().map(|p| p.total_length as u64).sum(),
        }
    }

    /// Throughput in bytes per second for this burst.
    pub fn throughput(&self) -> f64 {
        match self {
            Burst::Tcp(burst) => burst.throughput().unwrap_or(0.0),
            Burst::Udp(packets) => Self::get_throughput(packets),
            Burst::Other(packets) => Self::get_throughput(packets),
        }
    }
}

impl TcpBurst {
    /// Flatten into a `Vec<PacketType>` by concatenating all acked sub-bursts.
    pub fn flatten(self) -> Vec<PacketType> {
        self.packets
            .into_iter()
            .flat_map(|acked| acked.acked_packets)
            .collect()
    }

    /// Iterate over all packets in the burst without consuming it.
    pub fn iter(&self) -> impl Iterator<Item = &PacketType> {
        self.packets.iter().flat_map(|acked| acked.iter())
    }

    /// Sum of all packet lengths in this TCP burst.
    pub fn total_length(&self) -> u32 {
        self.packets.iter().map(|acked| acked.total_length).sum()
    }

    /// Duration from first packet sent to final ACK.
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

    /// Throughput of this TCP burst (bytes per second).
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

/// Represents a set of packets acknowledged together, with timing metadata.
#[derive(Debug)]
pub struct Acked {
    acked_packets: Vec<PacketType>,
    /// Time when the ACK was received.
    pub ack_time: SystemTime,
    first_sent_time: Option<SystemTime>,
    last_sent_time: SystemTime,
    /// Total length of all acked packets.
    pub total_length: u32,
}

impl Acked {
    /// Create a new `Acked` group from raw packet list and timing.
    fn from_acked(
        acked_packets: Vec<PacketType>,
        ack_time: SystemTime,
        first_sent_time: Option<SystemTime>,
    ) -> Self {
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

    /// Compute gap-in, gap-out, and payload length since last ACK.
    ///
    /// Returns `(gap_in_secs, gap_out_secs, total_payload_bytes)` if possible.
    pub fn get_gin_gout_len(&self, last_ack: SystemTime) -> Option<(f64, f64, u32)> {
        if let Some(first_sent_time) = self.first_sent_time {
            let gin = self.last_sent_time.duration_since(first_sent_time).ok()?;
            let gout = self.ack_time.duration_since(last_ack).ok()?;
            let total_length = self
                .acked_packets
                .iter()
                .map(|p| p.payload_len as u32)
                .sum::<u32>();
            Some((gin.as_secs_f64(), gout.as_secs_f64(), total_length))
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.acked_packets.len()
    }
}

/// Internal per-direction TCP state machine for building bursts.
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
    /// Update and return inter-packet gap since last sent packet.
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

    /// Update and return inter-ACK gap since last ACK.
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

    /// Register a packet into the TCP stream, possibly producing a completed burst.
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
            let last_sent: Option<SystemTime> =
                if let Some(prev_ack) = self.cur_burst.packets.last() {
                    Some(prev_ack.last_sent_time)
                } else {
                    None
                };
            self.cur_burst.packets.push(Acked::from_acked(
                acked_packets,
                packet.timestamp,
                last_sent,
            ));
        }
        self.last_registered = Some(packet.timestamp);
        ret
    }

    /// Track an outgoing packet by sequence number, handling retransmissions.
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
        for (&seq, sent_packet) in self.packets.iter_mut() {
            if seq_less_equal(seq.wrapping_add(sent_packet.payload_len as u32), ack) {
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

/// Tracks both directions of a TCP connection, producing bursts.
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

    /// Consume and return any accumulated bursts from both sides.
    /// Used for cleaning up after a connection is closed.
    pub fn take_bursts(&mut self) -> (Burst, Burst) {
        let sent = std::mem::take(&mut self.sent.cur_burst);
        let received = std::mem::take(&mut self.received.cur_burst);
        (sent.into(), received.into())
    }

    /// Register a packet, routing it to the proper `TcpStream`.
    ///
    /// Returns `(burst, direction)` if a burst completed.
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
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_seq_cmp_wraparound() {
        // 0 follows 0xFFFF_FFFF by wrap-around
        assert!(seq_cmp(0, u32::MAX) > 0);
        assert!(seq_cmp(u32::MAX, 0) < 0);
    }

    #[test]
    fn test_mem_swap() {
        let mut v = vec![1, 2, 3];
        let taken = std::mem::take(&mut v);
        assert_eq!(taken, vec![1, 2, 3], "taken value should be original contents");
        assert!(v.is_empty(), "original vector should now be empty");
    }

    #[test]
    fn test_sort_by_time() {
        #[derive(Clone)]
        struct P { sent_time: SystemTime }
        let t1 = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let t2 = SystemTime::UNIX_EPOCH + Duration::from_secs(2);
        let t3 = SystemTime::UNIX_EPOCH + Duration::from_secs(3);
        let mut pkts = vec![P { sent_time: t3 }, P { sent_time: t1 }, P { sent_time: t2 }];
        pkts.sort_by(|a, b| a.sent_time.cmp(&b.sent_time));
        assert_eq!(pkts[0].sent_time, t1);
        assert_eq!(pkts[2].sent_time, t3);
    }
}
