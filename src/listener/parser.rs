use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use super::analyzer::Analyzer;
use super::procfs_reader::{self, get_interface, get_interface_info};
use super::stream_id::TcpStreamId;
use super::stream_manager::TcpStreamManager;
use capture::OwnedPacket;
use log::{error, info};
use neli_wifi::{Bss, Interface, Station};
use pcap::Device;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::tcp::{TcpOptionIterable, TcpPacket};
use pnet::packet::udp::UdpPacket;
use pnet::packet::{ethernet::EthernetPacket, ip::IpNextHeaderProtocols, Packet};
use procfs::net::TcpState;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::time;

use super::capture;

#[derive(Debug)]
pub struct NetlinkData {
    pub stations: Vec<Station>, // Currently connected stations
    pub bss: Vec<Bss>, // BSS information
}

pub struct Parser {
    packet_stream: UnboundedReceiver<OwnedPacket>,
    own_ip: IpAddr,
    device_name: String,
    stream_manager: TcpStreamManager,
    netlink_data: Option<NetlinkData>,
    netstat_data: Option<HashMap<TcpStreamId, (TcpState, u32, u32, u64)>>,
    analyzer: Analyzer,
}

#[derive(Debug)]
pub enum TransportPacket {
    TCP {
        sequence: u32,
        acknowledgment: u32,
        flags: u8,
        // Maximum size of an IP packet is 65,535 bytes (2^16 - 1)
        payload_len: u16,
        tsval: Option<u32>,
        tsecr: Option<u32>,
    },
    UDP,
    ICMP,
}

/// time::Duration and SystemTime uses Nanosecond precision
pub fn tv_to_system_time(tv: libc::timeval) -> SystemTime {
    match super::Settings::PRESICION {
        pcap::Precision::Micro => {
            let dur = time::Duration::new(tv.tv_sec as u64, tv.tv_usec as u32 * 1000);
            UNIX_EPOCH + dur
        }
        pcap::Precision::Nano => {
            let dur = time::Duration::new(tv.tv_sec as u64, tv.tv_usec as u32);
            UNIX_EPOCH + dur
        }
    }
}

#[derive(Debug)]
pub struct ParsedPacket {
    pub src_ip: IpAddr,
    pub dst_ip: IpAddr,
    pub src_port: u16,
    pub dst_port: u16,
    pub transport: TransportPacket,
    pub total_length: u32,
    pub timestamp: SystemTime,
}


impl Parser {
    pub fn new(
        packet_stream: UnboundedReceiver<OwnedPacket>,
        device: Device,
    ) -> Self {

        // Attempt to find an IPv4 or IPv6 address
        let own_ip = device.addresses.iter().find_map(|addr| {
            match addr.addr {
                IpAddr::V4(ipv4) => Some(IpAddr::V4(ipv4)),
                IpAddr::V6(ipv6) => Some(IpAddr::V6(ipv6)),
            }
        }).expect("Device does not have an IPv4 or IPv6 address");

        Parser {
            packet_stream,
            own_ip,
            device_name: device.name,
            stream_manager: TcpStreamManager::new(super::Settings::TCP_STREAM_TIMEOUT),
            netlink_data: None,
            netstat_data: None,
            analyzer: Analyzer::new(),
        }
    }

    pub async fn start(mut self) {


        let interface = match get_interface(&self.device_name).await {
            Ok(interface) => {
                info!("Interface: {:?}", interface);
                interface
            }
            Err(e) => {
                error!("Error getting interface: {:?}", e);
                return;
            }
        };


        // Create the channel
        let (netlink_tx, mut netlink_rx) = mpsc::unbounded_channel();
        let (netstat_tx, mut netstat_rx) = mpsc::unbounded_channel();

        // Spawn netlink_comms and pass the sender
        let netlink_handle = tokio::spawn(async move {
            Parser::netlink_comms(netlink_tx, interface).await;
        });

        let netstat_handle = tokio::spawn(async move {
            Parser::periodic_netstat(netstat_tx).await;
        });

        let mut interval = time::interval(super::Settings::CLEANUP_INTERVAL);

        loop {
            tokio::select! {
                Some(packet) = self.packet_stream.recv() => {
                    // Handle the captured packet
                    self.handle_capture(packet);
                },
                Some(netlink_data) = netlink_rx.recv() => {
                    // Handle netlink data received from netlink_comms
                    self.handle_netlink_data(netlink_data);
                },
                Some(netstat) = netstat_rx.recv() => {
                    // Handle netstat data received from periodic_netstat
                    self.handle_netstat_data(netstat);
                },
                _ = interval.tick() => {
                    // Perform the periodic action here
                    //println!("Performing periodic action");
                    self.stream_manager.periodic(self.netstat_data.take());
                    if let Some(data) = &self.netlink_data {
                        // Print bitrate and signal strength
                        for station in &data.stations {
                            println!("Station: {:?}, Signal: {:?} dBm, RX {:?}, TX {:?}", station.bssid, station.signal, station.rx_bitrate, station.tx_bitrate);
                            println!("Station_info: {:?}", station);
                        }
                    }
                },
                else => {
                    // Both streams have ended
                    break;
                }
            }
        }

        // Optionally wait for netlink_comms to finish
        let _ = netlink_handle.await;
        let _ = netstat_handle.await;
    }

    pub async fn stop(self) {
        // Stop the parser
    }

    async fn periodic_netstat(netstat_tx: mpsc::UnboundedSender<HashMap<TcpStreamId, (TcpState, u32, u32, u64)>>) {
        loop {
            let netstat = procfs_reader::netstat_test_async().await;
            // Send a message to trigger the periodic action
            if netstat_tx.send(netstat).is_err() {
                break;
            }

            time::sleep(time::Duration::from_secs(3)).await;
        }

    }

    async fn netlink_comms(netlink_tx: mpsc::UnboundedSender<NetlinkData>, interface: Interface) {
        loop {
            // Obtain the data you want to send
            let data = get_interface_info(interface.index.unwrap()).await;

            if netlink_tx.send(data.unwrap()).is_err() {
                break;
            }

            time::sleep(time::Duration::from_secs(3)).await;
        }
    }

    /// Placeholder for handling netlink data
    fn handle_netlink_data(&mut self, data: NetlinkData) {
        self.netlink_data = Some(data);
    }

    /// Placeholder for handling netstat data
    fn handle_netstat_data(&mut self, data: HashMap<TcpStreamId, (TcpState, u32, u32, u64)>) {
        self.netstat_data = Some(data);
    }

    fn handle_capture(&mut self, packet: OwnedPacket) {
        // Handle the captured packet
        self.analyzer.process_packet(&packet);

        let parsed_packet = match self.parse_packet(&packet) {
            Some(packet) => packet,
            None => return,
        };

        match &parsed_packet.transport {
            TransportPacket::TCP { .. } => {
                if let Some(rtt) = self.stream_manager.record_packet(&parsed_packet, self.own_ip) {
                    println!("RTT: {:?}, SRC: {:?}, DST: {:?}", rtt, parsed_packet.src_ip, parsed_packet.dst_ip);
                }
            }
            TransportPacket::UDP => {
                // Handle UDP packet
            }
            TransportPacket::ICMP => {
                // Handle ICMP packet
                //println!("ICMP packet received");
            }
        }
    }

    /* Parses an `OwnedPacket` into a `ParsedPacket`.
     * Returns `Some(ParsedPacket)` if parsing is successful, otherwise `None`.
     */
    pub fn parse_packet(&self, packet: &OwnedPacket) -> Option<ParsedPacket> {
        // Parse the Ethernet frame
        let total_length = packet.header.len;
        let eth = EthernetPacket::new(&packet.data)?;

        match eth.get_ethertype() {
            pnet::packet::ethernet::EtherTypes::Ipv4 => {
                self.parse_ipv4_packet(eth.payload(), total_length, tv_to_system_time(packet.header.ts))
            }
            pnet::packet::ethernet::EtherTypes::Ipv6 => {
                self.parse_ipv6_packet(eth.payload(), total_length, tv_to_system_time(packet.header.ts))
            }
            _ => None,
        }
    }


    fn parse_ipv4_packet(
        &self,
        payload: &[u8],
        total_length: u32,
        timestamp: SystemTime,
    ) -> Option<ParsedPacket> {
        let ipv4 = Ipv4Packet::new(payload)?;
        let protocol = ipv4.get_next_level_protocol();

        match protocol {
            IpNextHeaderProtocols::Tcp => {
                self.parse_tcp_packet(
                    ipv4.payload(),
                    IpAddr::V4(ipv4.get_source()),
                    IpAddr::V4(ipv4.get_destination()),
                    total_length,
                    timestamp,
                )
            }
            IpNextHeaderProtocols::Udp => {
                self.parse_udp_packet(
                    ipv4.payload(),
                    IpAddr::V4(ipv4.get_source()),
                    IpAddr::V4(ipv4.get_destination()),
                    total_length,
                    timestamp,
                )
            }
            IpNextHeaderProtocols::Icmp => {
                Some(ParsedPacket {
                    src_ip: IpAddr::V4(ipv4.get_source()),
                    dst_ip: IpAddr::V4(ipv4.get_destination()),
                    src_port: 0,
                    dst_port: 0,
                    transport: TransportPacket::ICMP,
                    total_length,
                    timestamp,
                })
            }
            _ => {
                println!("Unknown protocol: {:?}", protocol);
                None
            },
        }
    }

///! Should probably fix this part.

    fn parse_ipv6_packet(
        &self,
        payload: &[u8],
        total_length: u32,
        timestamp: SystemTime,
    ) -> Option<ParsedPacket> {
        let ipv6 = Ipv6Packet::new(payload)?;
        let protocol = ipv6.get_next_header();

        match protocol {
            IpNextHeaderProtocols::Tcp => {
                self.parse_tcp_packet(
                    ipv6.payload(),
                    IpAddr::V6(ipv6.get_source()),
                    IpAddr::V6(ipv6.get_destination()),
                    total_length,
                    timestamp,
                )
            }
            IpNextHeaderProtocols::Udp => {
                self.parse_udp_packet(
                    ipv6.payload(),
                    IpAddr::V6(ipv6.get_source()),
                    IpAddr::V6(ipv6.get_destination()),
                    total_length,
                    timestamp,
                )
            }
            IpNextHeaderProtocols::Icmpv6 => {
                Some(ParsedPacket {
                    src_ip: IpAddr::V6(ipv6.get_source()),
                    dst_ip: IpAddr::V6(ipv6.get_destination()),
                    src_port: 0,
                    dst_port: 0,
                    transport: TransportPacket::ICMP,
                    total_length,
                    timestamp,
                })
            }
            _ => None,
        }
    }

    fn parse_timestamp(&self, tcp_options : TcpOptionIterable) -> Option<(u32, u32)> {
        for option in tcp_options {
            if option.get_number() == pnet::packet::tcp::TcpOptionNumbers::TIMESTAMPS {
                let timestamp_bytes = option.payload();

                // Ensure the timestamp option payload is 8 bytes (TSval + TSecr)
                if timestamp_bytes.len() != 8 {
                    // println!("Invalid timestamp length");
                    continue;
                }
                let tsval = u32::from_be_bytes([
                    timestamp_bytes[0],
                    timestamp_bytes[1],
                    timestamp_bytes[2],
                    timestamp_bytes[3],
                ]);
                let tsecr = u32::from_be_bytes([
                    timestamp_bytes[4],
                    timestamp_bytes[5],
                    timestamp_bytes[6],
                    timestamp_bytes[7],
                ]);

                //println!("TSval: {}, TSecr: {}", tsval, tsecr);

                return Some((tsval, tsecr));
            }
        }
        None
    }

    fn parse_tcp_packet(
        &self,
        payload: &[u8],
        src_ip: IpAddr,
        dst_ip: IpAddr,
        total_length: u32,
        timestamp: SystemTime,
    ) -> Option<ParsedPacket> {
        let tcp = TcpPacket::new(payload)?;
        // Print timestamp if TCP timestamp option is present
        let (tsval, tsecr) = match self.parse_timestamp(tcp.get_options_iter()) {
            Some((tsval, tsecr)) => (tsval, tsecr),
            None => (0, 0),
        };

        Some(ParsedPacket {
            src_ip,
            dst_ip,
            src_port: tcp.get_source(),
            dst_port: tcp.get_destination(),
            transport: TransportPacket::TCP {
                sequence: tcp.get_sequence(),
                acknowledgment: tcp.get_acknowledgement(),
                flags: tcp.get_flags(),
                payload_len: tcp.payload().len() as u16,
                tsval: Some(tsval),
                tsecr: Some(tsecr),
            },
            total_length,
            timestamp,
        })
    }

    fn parse_udp_packet(
        &self,
        payload: &[u8],
        src_ip: IpAddr,
        dst_ip: IpAddr,
        total_length: u32,
        timestamp: SystemTime,
    ) -> Option<ParsedPacket> {
        let udp = UdpPacket::new(payload)?;

        Some(ParsedPacket {
            src_ip,
            dst_ip,
            src_port: udp.get_source(),
            dst_port: udp.get_destination(),
            transport: TransportPacket::UDP,
            total_length,
            timestamp,
        })
    }
}
