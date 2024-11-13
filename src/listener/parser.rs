use std::net::IpAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use super::analyzer::Analyzer;
use super::procfs_reader::{self, get_interface, get_interface_info, NetStat};
use super::stream_manager::StreamManager;
use capture::OwnedPacket;
use log::{error, info};
use neli_wifi::{Bss, Interface, Station};
use pcap::Device;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::tcp::{TcpOptionIterable, TcpPacket};
use pnet::packet::udp::UdpPacket;
use pnet::packet::{ethernet::EthernetPacket, ip::IpNextHeaderProtocols, Packet};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::time;
use anyhow::{Result, Context};
use tokio::sync::mpsc::{Receiver, Sender};

const CHANNEL_CAPACITY: usize = 1000;

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
    stream_manager: StreamManager,
    netlink_data: Vec<NetlinkData>,
    netstat_data: Option<NetStat>,
    analyzer: Analyzer,
}

#[derive(Debug)]
pub enum TransportPacket {
    TCP {
        sequence: u32,
        acknowledgment: u32,
        flags: TcpFlags,
        // Maximum size of an IP packet is 65,535 bytes (2^16 - 1)
        payload_len: u16,
        tsval: Option<u32>,
        tsecr: Option<u32>,
        src_port: u16,
        dst_port: u16,
    },
    UDP {
        src_port: u16,
        dst_port: u16,
    },
    ICMP,
    OTHER {
        protocol: u8,
    },
}

#[derive(Debug)]
pub struct TcpFlags(u8);

impl TcpFlags {
    const SYN: u8 = 0x02;
    const ACK: u8 = 0x10;
    const FIN: u8 = 0x01;

    fn is_syn(&self) -> bool {
        self.0 & Self::SYN != 0
    }

    fn is_ack(&self) -> bool {
        self.0 & Self::ACK != 0
    }

    fn is_fin(&self) -> bool {
        self.0 & Self::FIN != 0
    }
}

impl TransportPacket {
    pub fn is_syn(&self) -> bool {
        matches!(self, TransportPacket::TCP { flags, .. } if flags.is_syn())
    }

    pub fn is_ack(&self) -> bool {
        matches!(self, TransportPacket::TCP { flags, .. } if flags.is_ack())
    }

    pub fn is_fin(&self) -> bool {
        matches!(self, TransportPacket::TCP { flags, .. } if flags.is_fin())
    }
}

/// time::Duration and SystemTime uses Nanosecond precision
pub fn timeval_to_system_time(tv: libc::timeval) -> SystemTime {
    match super::Settings::PRECISION {
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
    pub transport: TransportPacket,
    pub total_length: u32,
    pub timestamp: SystemTime,
}


impl Parser {
    pub fn new(
        packet_stream: UnboundedReceiver<OwnedPacket>,
        device: Device,
    ) -> Result<Self> {
        let own_ip = device.addresses.iter()
            .filter_map(|addr| match addr.addr {
                IpAddr::V4(ipv4) => Some(IpAddr::V4(ipv4)),
                IpAddr::V6(ipv6) => Some(IpAddr::V6(ipv6)),
            })
            .next()
            .context("Device does not have an IPv4 or IPv6 address")?;

        Ok(Parser {
            packet_stream,
            own_ip,
            device_name: device.name,
            stream_manager: StreamManager::default(),
            netlink_data: Vec::new(),
            netstat_data: None,
            analyzer: Analyzer::new(),
        })
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


        // Create bounded channels
        let (netlink_tx, mut netlink_rx):
            (Sender<NetlinkData>, Receiver<NetlinkData>)
            = mpsc::channel(CHANNEL_CAPACITY);

        let (netstat_tx, mut netstat_rx):
            (Sender<NetStat>, Receiver<_>)
            = mpsc::channel(CHANNEL_CAPACITY);

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
                    self.handle_capture(packet);
                },
                Some(netlink_data) = netlink_rx.recv() => {
                    self.handle_netlink_data(netlink_data);
                },
                Some(netstat_data) = netstat_rx.recv() => {
                    self.handle_netstat_data(netstat_data);
                },
                _ = interval.tick() => {
                    self.stream_manager.periodic(self.netstat_data.take());
                },
                else => {
                    // Both streams have ended
                    break;
                }
            }
        }

        let _ = netlink_handle.await;
        let _ = netstat_handle.await;
    }

    pub async fn stop(self) {
        // Stop the parser
    }

    async fn periodic_netstat(
        netstat_tx: Sender<NetStat>,
    ) {
        loop {
            let netstat = procfs_reader::proc_net().await;

            if netstat_tx.send(netstat).await.is_err() {
                break;
            }

            time::sleep(time::Duration::from_secs(7)).await;
        }
    }

    async fn netlink_comms(netlink_tx: Sender<NetlinkData>, interface: Interface) {
        loop {
            let data = get_interface_info(interface.index.unwrap()).await;

            if netlink_tx.send(data.unwrap()).await.is_err() {
                break;
            }

            time::sleep(time::Duration::from_secs(7)).await;
        }
    }

    /// Placeholder for handling netlink data
    fn handle_netlink_data(&mut self, data: NetlinkData) {
        self.netlink_data.push(data);
        if self.netlink_data.len() > 10 {
            self.netlink_data.remove(0);
        }
    }

    /// Placeholder for handling netstat data
    fn handle_netstat_data(&mut self, data: NetStat) {
        self.netstat_data = Some(data);
    }

    fn handle_capture(&mut self, packet: OwnedPacket) {
        // Handle the captured packet
        self.analyzer.process_packet(&packet);

        let parsed_packet = match self.parse_packet(&packet) {
            Some(packet) => packet,
            None => return,
        };

        self.stream_manager.record_ip_packet(parsed_packet, self.own_ip);
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
                self.parse_ipv4_packet(eth.payload(), total_length, timeval_to_system_time(packet.header.ts))
            }
            pnet::packet::ethernet::EtherTypes::Ipv6 => {
                self.parse_ipv6_packet(eth.payload(), total_length, timeval_to_system_time(packet.header.ts))
            }
            _ => None,
        }
    }

    fn parse_ip_packet (
        &self,
        payload: &[u8],
        src_ip: IpAddr,
        dst_ip: IpAddr,
        total_length: u32,
        timestamp: SystemTime,
        protocol: pnet::packet::ip::IpNextHeaderProtocol,
    ) -> Option<ParsedPacket> {
        match protocol {
            IpNextHeaderProtocols::Tcp => {
                self.parse_tcp_packet(
                    payload,
                    src_ip,
                    dst_ip,
                    total_length,
                    timestamp,
                )
            }
            IpNextHeaderProtocols::Udp => {
                self.parse_udp_packet(
                    payload,
                    src_ip,
                    dst_ip,
                    total_length,
                    timestamp,
                )
            }
            IpNextHeaderProtocols::Icmp => {
                Some(ParsedPacket {
                    src_ip,
                    dst_ip,
                    transport: TransportPacket::ICMP,
                    total_length,
                    timestamp,
                })
            }
            _ => {
                Some(ParsedPacket {
                    src_ip,
                    dst_ip,
                    transport: TransportPacket::OTHER {
                        protocol: protocol.0,
                    },
                    total_length,
                    timestamp,
                })
            },
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

        self.parse_ip_packet(
            ipv4.payload(),
            IpAddr::V4(ipv4.get_source()),
            IpAddr::V4(ipv4.get_destination()),
            total_length,
            timestamp,
            protocol,
        )
    }

    fn parse_ipv6_packet(
        &self,
        payload: &[u8],
        total_length: u32,
        timestamp: SystemTime,
    ) -> Option<ParsedPacket> {
        let ipv6 = Ipv6Packet::new(payload)?;
        let protocol = ipv6.get_next_header();

        self.parse_ip_packet(
            ipv6.payload(),
            IpAddr::V6(ipv6.get_source()),
            IpAddr::V6(ipv6.get_destination()),
            total_length,
            timestamp,
            protocol,
        )
    }

    fn parse_timestamp(&self, tcp_options: TcpOptionIterable) -> Option<(u32, u32)> {
        for option in tcp_options {
            if option.get_number() == pnet::packet::tcp::TcpOptionNumbers::TIMESTAMPS {
                let timestamp_bytes = option.payload();

                if timestamp_bytes.len() != 8 {
                    log::warn!("Invalid timestamp length: expected 8, got {}", timestamp_bytes.len());
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
            transport: TransportPacket::TCP {
                sequence: tcp.get_sequence(),
                acknowledgment: tcp.get_acknowledgement(),
                flags: TcpFlags(tcp.get_flags()),
                payload_len: tcp.payload().len() as u16,
                tsval: Some(tsval),
                tsecr: Some(tsecr),
                src_port: tcp.get_source(),
                dst_port: tcp.get_destination(),
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
            transport: TransportPacket::UDP {
                src_port: udp.get_source(),
                dst_port: udp.get_destination(),
            },
            total_length,
            timestamp,
        })
    }
}
