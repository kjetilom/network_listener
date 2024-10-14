use std::net::IpAddr;
use std::time::{Instant, Duration};

use super::stream_id::Connection;
use super::stream_manager::TcpStreamManager;
use capture::OwnedPacket;
use log::{debug, info};
use pcap::{Address, Device, PacketHeader};
use pnet::packet::ethernet::EtherTypes;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::tcp::TcpPacket;
use pnet::packet::udp::UdpPacket;
use pnet::packet::{ethernet::EthernetPacket, ip::IpNextHeaderProtocols, Packet};
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
    tcp: u64,
    udp: u64,
    other: u64,
    ipv4: u64,
    ipv6: u64,
}

impl PacketStats {
    fn new() -> Self {
        PacketStats {
            total_bytes: 0,
            total_packets: 0,
            start_time: Instant::now(),
            measurement_interval: MEASUREMENT_INTERVAL,
            tcp: 0,
            udp: 0,
            other: 0,
            ipv4: 0,
            ipv6: 0,
        }
    }

    fn reset(&mut self) {
        self.total_bytes = 0;
        self.total_packets = 0;
        self.start_time = Instant::now();
        self.tcp = 0;
        self.udp = 0;
        self.other = 0;
        self.ipv4 = 0;
        self.ipv6 = 0;
    }
}

pub struct TrafficAnalyzer {
    packet_stream: UnboundedReceiver<OwnedPacket>,
    local_addrs: Vec<Address>, // Vector of addresses belonging to the local device
    stream_manager: TcpStreamManager,
    stats: PacketStats,
}

#[derive(Debug)]
pub enum Direction {
    Incoming,
    Outgoing,
}

// Representation of a parsed PCAP packet.
#[derive(Debug)]
pub struct ParsedPacket<'a> {
    pub src_ip: IpAddr,
    pub dst_ip: IpAddr,
    pub timestamp: Timeval,
    pub total_length: u32,
    pub protocol: ProtocolPacket<'a>,
    pub direction: Direction,
}

#[derive(Debug)]
pub enum ProtocolPacket<'a> {
    TCP(TcpPacket<'a>),
    UDP(UdpPacket<'a>),
    Other(&'a [u8]),
}

impl ProtocolPacket<'_> {
    pub fn protocol(&self) -> Protocol {
        match self {
            ProtocolPacket::TCP(_) => Protocol::TCP,
            ProtocolPacket::UDP(_) => Protocol::UDP,
            ProtocolPacket::Other(_) => Protocol::Other,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Hash, Clone)]
pub enum Protocol {
    TCP,
    UDP,
    Other,
}

impl<'a> ParsedPacket<'a> {
    pub fn from(
        packet: &'a OwnedPacket,
        addrs: &'a [Address],
    ) -> Option<Self> {
        // Parse pcap header
        let header = packet.header;
        let eth = EthernetPacket::new(&packet.data)?;

        match eth.get_ethertype() {
            EtherTypes::Ipv4 => Self::handle_ipv4(eth, header, addrs),
            EtherTypes::Ipv6 => Self::handle_ipv6(eth, header, addrs),
            _ => None,
        }
    }

    fn handle_ipv4(
        eth: EthernetPacket<'a>,
        header: PacketHeader,
        addrs: &[Address],
    ) -> Option<Self> {
        let ipv4 = Ipv4Packet::new(eth.payload())?;
        let next_level = ipv4.get_next_level_protocol();

        let protocol = match next_level {
            IpNextHeaderProtocols::Tcp => {
                match TcpPacket::new(ipv4.payload()) {
                    Some(tcp) => ProtocolPacket::TCP(tcp),
                    None => ProtocolPacket::Other(ipv4.payload()),
                }
            },
            IpNextHeaderProtocols::Udp => {
                match UdpPacket::new(ipv4.payload()) {
                    Some(udp) => ProtocolPacket::UDP(udp),
                    None => ProtocolPacket::Other(ipv4.payload()),
                }
            },
            _ => ProtocolPacket::Other(ipv4.payload()),
        };

        let src_ip = IpAddr::V4(ipv4.get_source());
        let direction = Self::get_direction(addrs, src_ip);

        Some(Self {
            src_ip,
            dst_ip: IpAddr::V4(ipv4.get_destination()),
            timestamp: Timeval(header.ts),
            total_length: header.len,
            protocol,
            direction,
        })
    }

    fn handle_ipv6(
        eth: EthernetPacket<'a>,
        header: PacketHeader,
        addrs: &'a [Address],
    ) -> Option<Self> {
        let ipv6 = Ipv6Packet::new(eth.payload())?;
        let next_level = ipv6.get_next_header();

        let protocol = match next_level {
            IpNextHeaderProtocols::Tcp => {
                match TcpPacket::new(ipv6.payload()) {
                    Some(tcp) => ProtocolPacket::TCP(tcp),
                    None => ProtocolPacket::Other(ipv6.payload()),
                }
            },
            IpNextHeaderProtocols::Udp => {
                match UdpPacket::new(ipv6.payload()) {
                    Some(udp) => ProtocolPacket::UDP(udp),
                    None => ProtocolPacket::Other(ipv6.payload()),
                }
            },
            _ => ProtocolPacket::Other(ipv6.payload()),
        };

        let src_ip = IpAddr::V6(ipv6.get_source());
        let dst_ip = IpAddr::V6(ipv6.get_destination());
        let direction = Self::get_direction(addrs, src_ip);

        Some(Self {
            src_ip,
            dst_ip,
            timestamp: Timeval(header.ts),
            total_length: header.len,
            protocol,
            direction,
        })
    }

    fn get_direction(addrs: &[Address], src_ip: IpAddr) -> Direction {
        if addrs.iter().any(|addr| addr.addr == src_ip) {
            Direction::Outgoing
        } else {
            Direction::Incoming
        }
    }

    pub fn as_tcp(&self) -> Option<&TcpPacket<'a>> {
        match &self.protocol {
            ProtocolPacket::TCP(ref tcp_packet) => Some(tcp_packet),
            _ => None,
        }
    }

    /// Returns a reference to the inner `UdpPacket` if this is a UDP packet.
    pub fn as_udp(&self) -> Option<&UdpPacket<'a>> {
        match &self.protocol {
            ProtocolPacket::UDP(ref udp_packet) => Some(udp_packet),
            _ => None,
        }
    }

    pub fn src_port(&self) -> u16 {
        match &self.protocol {
            ProtocolPacket::TCP(tcp) => tcp.get_source(),
            ProtocolPacket::UDP(udp) => udp.get_source(),
            _ => 0,
        }
    }

    pub fn dst_port(&self) -> u16 {
        match &self.protocol {
            ProtocolPacket::TCP(tcp) => tcp.get_destination(),
            ProtocolPacket::UDP(udp) => udp.get_destination(),
            _ => 0,
        }
    }

    pub fn connection(&self) -> Option<Connection> {
        Connection::from(self)
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

impl TrafficAnalyzer {
    pub fn new(packet_stream: UnboundedReceiver<OwnedPacket>, device: Device) -> Self {
        let local_addrs = device.addresses;
        let stats = PacketStats::new();

        TrafficAnalyzer {
            packet_stream,
            local_addrs,
            stream_manager: TcpStreamManager::new(tracker::TIMEOUT),
            stats,
        }
    }

    fn handle_tcp(&mut self, parsed_packet: &ParsedPacket) {

    }

    fn handle_udp(&self, parsed_packet: &ParsedPacket) {
        // Handle UDP-specific logic
        info!("UDP packet: {:?}", parsed_packet);
    }

    pub async fn start(mut self) {
        while let Some(packet) = self.packet_stream.recv().await {
            // Register the packet statistics
            self.reg_packet(&packet);

            let parsed_packet = match ParsedPacket::from(&packet, &self.local_addrs) {
                Some(parsed_packet) => parsed_packet,
                None => {
                    debug!("Parsing of packet failed");
                    continue;
                }
            };

            // Register parsed packet statistics
            match parsed_packet.protocol.protocol() {
                Protocol::TCP => self.stats.tcp += 1,
                Protocol::UDP => self.stats.udp += 1,
                Protocol::Other => self.stats.other += 1,
            }

            match parsed_packet.src_ip {
                IpAddr::V4(_) => self.stats.ipv4 += 1,
                IpAddr::V6(_) => self.stats.ipv6 += 1,
            }

            match parsed_packet.protocol.protocol() {
                Protocol::TCP => {
                    self.stream_manager.record_sent(&parsed_packet);

                    if let Some(duration) = self.stream_manager.record_ack(&parsed_packet) {
                        println!(
                            "RTT: {:?}, Source: {:?}, Destination: {:?}",
                            duration, parsed_packet.src_ip, parsed_packet.dst_ip
                        );
                    }
                }
                Protocol::UDP => self.handle_udp(&parsed_packet),
                Protocol::Other => {
                    ()
                }
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
                self.stats.tcp,
                mbps,
                elapsed,
            );

            self.stats.reset();
        }

        self.stats.total_bytes += packet.header.len as u64;
        self.stats.total_packets += 1;
    }
}
