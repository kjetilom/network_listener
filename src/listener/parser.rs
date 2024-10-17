use std::net::IpAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use super::analyzer::Analyzer;
use super::stream_manager::TcpStreamManager;
use capture::OwnedPacket;
use pcap::Device;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::tcp::{TcpOptionIterable, TcpPacket};
use pnet::packet::udp::UdpPacket;
use pnet::packet::{ethernet::EthernetPacket, ip::IpNextHeaderProtocols, Packet};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time;

use super::capture;

pub struct Parser {
    packet_stream: UnboundedReceiver<OwnedPacket>,
    own_ip: IpAddr,
    stream_manager: TcpStreamManager,
}

#[derive(Debug)]
pub enum TransportPacket {
    TCP {
        sequence: u32,
        acknowledgment: u32,
        flags: u8,
        tsval: u32,
        tsecr: u32,
    },
    UDP,
}

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
    pub fn new(packet_stream: UnboundedReceiver<OwnedPacket>, device: Device) -> Self {
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
            stream_manager: TcpStreamManager::new(super::Settings::TCP_STREAM_TIMEOUT),
        }
    }

    pub async fn start(mut self) {
        let mut analyzer = Analyzer::new();

        while let Some(packet) = self.packet_stream.recv().await {
            analyzer.process_packet(&packet);

            let parsed_packet = match self.parse_packet(&packet) {
                Some(packet) => packet,
                None => continue,
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
            _ => None,
        }
    }

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
                tsval,
                tsecr,
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
