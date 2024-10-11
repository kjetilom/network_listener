use std::net::{IpAddr, Ipv4Addr};
use std::time::{Instant, Duration};

use super::stream_manager::TcpStreamManager;
use capture::OwnedPacket;
use log::info;
use pcap::{Address, Device};
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::tcp::TcpPacket;
use pnet::packet::udp::UdpPacket;
use pnet::packet::{ethernet::EthernetPacket, ip::IpNextHeaderProtocols, Packet};
use serde::de;
use tokio::sync::mpsc::UnboundedReceiver;

use super::capture;
use super::tracker;

// The interval at which to measure the network traffic
static MEASUREMENT_INTERVAL : Duration = Duration::from_secs(1);

struct PacketStats {
    total_bytes: u64,
    total_packets: u64,
    start_time: Instant,
    measurement_interval: Duration, // Reset interval (in seconds)
    tcp_packets: u64,
}

impl PacketStats {
    fn new() -> Self {
        PacketStats {
            total_bytes: 0,
            total_packets: 0,
            start_time: Instant::now(),
            measurement_interval: MEASUREMENT_INTERVAL,
            tcp_packets: 0,
        }
    }

    fn reset(&mut self) {
        self.total_bytes = 0;
        self.total_packets = 0;
        self.start_time = Instant::now();
        self.tcp_packets = 0;
    }
}

pub struct Parser {
    packet_stream: UnboundedReceiver<OwnedPacket>,
    local_addrs: Vec<Address>,
    stream_manager: TcpStreamManager,
    stats: PacketStats,
}

pub enum Direction {
    Incoming,
    Outgoing,
}

impl Direction {
    pub fn from(packet: &ParsedPacket, dev: &Device) -> Self {
        match dev.addresses.iter().find(|addr| addr.addr == packet.src_ip) {
            Some(_) => Direction::Outgoing,
            None => Direction::Incoming,
        }
    }
}

#[derive(Debug)]
pub struct ParsedPacket<'a> {
    pub src_ip: IpAddr,
    pub dst_ip: IpAddr,
    pub protocol: Protocol<'a>,
    // pub src_port: u16,
    // pub dst_port: u16,
    // pub sequence: u32,
    // pub acknowledgment: u32,
    // pub flags: u8,
    // pub total_length: u32,
    // pub timestamp: Timeval,
}

#[derive(Debug)]
pub enum Protocol<'a> {
    Tcp(TcpPacket<'a>),
    Udp(UdpPacket<'a>),
    Other(&'a [u8]),
}


impl Protocol<'_> {
    pub fn payload(&self) -> &[u8] {
        match self {
            Protocol::Tcp(tcp) => tcp.payload(),
            Protocol::Udp(udp) => udp.payload(),
            Protocol::Other(payload) => payload,
        }
    }
}

impl Default for Protocol<'_> {
    fn default() -> Self {
        Protocol::Other(&[])
    }
}

// Implement something similar to the following: if tcp is not available, return an error



// pub struct TcpPacket {
//     pub packet: ParsedPacket,
//     pub sequence: u32,
//     pub acknowledgment: u32,
//     pub flags: u8,
// }

impl ParsedPacket<'_> {

    pub fn from<'a> (
        packet: &'a OwnedPacket,
        local_addrs: &'a Vec<Address>,
    ) -> Option<ParsedPacket<'a>> {
        // Parse pcap header
        let total_length = packet.header.len;
        let eth = EthernetPacket::new(&packet.data)?;

        match eth.get_ethertype() {
            ipv4 => ParsedPacket::handle_ipv4(eth),
            ipv6 => ParsedPacket::handle_ipv6(eth),
            _ => None,
        }
    }

    fn handle_ipv4(eth: EthernetPacket) -> Option<ParsedPacket> {
        let ipv4 = match Ipv4Packet::new(eth.payload()) {
            Some(ipv4) => ipv4,
            None => return None,
        };
        let next_level = ipv4.get_next_level_protocol();

        let protocol = match next_level {
            IpNextHeaderProtocols::Tcp =>
                match TcpPacket::new(ipv4.payload()) {
                    Some(tcp) => Protocol::Tcp(tcp),
                    None => Protocol::Other(ipv4.payload()),
                },
            IpNextHeaderProtocols::Udp =>
                match UdpPacket::new(ipv4.payload()) {
                    Some(udp) => Protocol::Udp(udp),
                    None => Protocol::Other(ipv4.payload()),
            },
            _ => Protocol::Other(ipv4.payload())
        };
        Some(ParsedPacket {
            src_ip: IpAddr::from(ipv4.get_source()),
            dst_ip: IpAddr::from(ipv4.get_destination()),
            protocol: protocol,
        })
    }

    fn handle_ipv6(eth: EthernetPacket) -> Option<ParsedPacket> {
        let ipv6 = match Ipv6Packet::new(eth.payload()) {
            Some(ipv6) => ipv6,
            None => return None,
        };
        let next_level = ipv6.get_next_header();
        let src_ip = ipv6.get_source().to_canonical();
        let dst_ip = ipv6.get_destination().to_canonical();
        let ttl = ipv6.get_hop_limit();

        let protocol = match next_level {
            IpNextHeaderProtocols::Tcp =>
                match TcpPacket::new(ipv6.payload()) {
                    Some(tcp) => Protocol::Tcp(tcp),
                    None => Protocol::Other(ipv6.payload()),
                },
            IpNextHeaderProtocols::Udp =>
                match UdpPacket::new(ipv6.payload()) {
                    Some(udp) => Protocol::Udp(udp),
                    None => Protocol::Other(ipv6.payload()),
            },
            _ => Protocol::Other(ipv6.payload())
        };

        Some(ParsedPacket {
            src_ip: src_ip,
            dst_ip: dst_ip,
            protocol: protocol,
        })
    }
}

#[derive(Clone, Copy)]
pub struct Timeval(libc::timeval);

impl std::fmt::Debug for Timeval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{:06}", self.0.tv_sec, self.0.tv_usec)
    }
}

impl std::ops::Sub for Timeval {
    type Output = Duration;

    fn sub(self, other: Timeval) -> Duration {
        Duration::new(
            (self.0.tv_sec - other.0.tv_sec) as u64,
            ((self.0.tv_usec - other.0.tv_usec) * 1000) as u32,
        )
    }
}

impl Parser {
    pub fn new(packet_stream: UnboundedReceiver<OwnedPacket>, device: Device) -> Self {
        let own_ip = match device.addresses[0].addr {
            IpAddr::V4(ip) => ip,
            _ => panic!("Device does not have an IPv4 address"),
        };
        let stats = PacketStats::new();

        Parser {
            packet_stream,
            own_ip: own_ip,
            stream_manager: TcpStreamManager::new(tracker::TIMEOUT),
            stats,
        }
    }

    pub async fn start(mut self) {
        while let Some(packet) = self.packet_stream.recv().await {
            // TODO: Move this to a separate thread
            self.reg_packet(&packet);

            let parsed_packet = match ParsedPacket::from(&packet) {
                Some(packet) => packet,
                None => continue,
            };

            self.reg_parsed();

            self.stream_manager.record_sent(
                &parsed_packet,
                self.own_ip,
            );

            match self.stream_manager.record_ack(&parsed_packet) {
                Some(duration) => {
                    println!(
                        "RTT: {:?}, Source: {:?}, Destination: {:?}",
                        duration, parsed_packet.src_ip, parsed_packet.dst_ip
                    );
                }
                None => {}
            }
        }
    }

    pub fn reg_packet(&mut self, packet: &OwnedPacket) {
        if self.stats.start_time.elapsed() >= self.stats.measurement_interval {
            let elapsed = self.stats.start_time.elapsed().as_secs_f64();
            let mbps = self.stats.total_bytes as f64 * 8.0 / 1_000_000.0 / elapsed;
            info!(
                "Packets: {} (TCP+IPv4 {}) | Mbps: {:.2} | Time elapsed: {:.2}s",
                self.stats.total_packets,
                self.stats.tcp_packets,
                mbps,
                elapsed,
            );

            self.stats.reset();
        }

        self.stats.total_bytes += packet.header.len as u64;
        self.stats.total_packets += 1;
    }

    pub fn reg_parsed(&mut self) {
        self.stats.tcp_packets += 1;
    }
}
