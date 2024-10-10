use std::net::{IpAddr, Ipv4Addr};

use super::analyzer::Analyzer;
use super::stream_manager::TcpStreamManager;
use capture::OwnedPacket;
use pcap::Device;
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::tcp::TcpPacket;
use pnet::packet::{ethernet::EthernetPacket, ip::IpNextHeaderProtocols, Packet};
use tokio::sync::mpsc::UnboundedReceiver;

use super::capture;
use super::tracker;

pub struct Parser {
    packet_stream: UnboundedReceiver<OwnedPacket>,
    own_ip: Ipv4Addr,
    stream_manager: TcpStreamManager,
}

pub struct ParsedPacket {
    pub src_ip: Ipv4Addr,
    pub dst_ip: Ipv4Addr,
    pub src_port: u16,
    pub dst_port: u16,
    pub sequence: u32,
    pub acknowledgment: u32,
    pub flags: u8,
    pub total_length: u32,
    pub timestamp: libc::timeval,
}

impl std::fmt::Debug for ParsedPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedPacket")
            .field("src_ip", &self.src_ip)
            .field("dst_ip", &self.dst_ip)
            .field("src_port", &self.src_port)
            .field("dst_port", &self.dst_port)
            .field("sequence", &self.sequence)
            .field("acknowledgment", &self.acknowledgment)
            .field("flags", &self.flags)
            .field("total_length", &self.total_length)
            .finish()
    }
}

impl Parser {
    pub fn new(packet_stream: UnboundedReceiver<OwnedPacket>, device: Device) -> Self {

        let own_ip = match device.addresses[0].addr {
            IpAddr::V4(ip) => ip,
            _ => panic!("Device does not have an IPv4 address"),
        };

        Parser {
            packet_stream,
            own_ip: own_ip,
            stream_manager: TcpStreamManager::new(tracker::TIMEOUT),
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

            self.stream_manager.record_sent_packet(
                &parsed_packet,
                &parsed_packet.sequence,
                self.own_ip,
            );

            match self.stream_manager.record_ack_packet(&parsed_packet) {
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

    /* Parses an `OwnedPacket` into a `ParsedPacket`.
     * Returns `Some(ParsedPacket)` if parsing is successful, otherwise `None`.
     */
    pub fn parse_packet(&self, packet: &OwnedPacket) -> Option<ParsedPacket> {
        // Parse the Ethernet frame
        let total_length = packet.header.len;
        let eth = EthernetPacket::new(&packet.data)?;

        // For now, we only care about IPv4 packets
        if eth.get_ethertype() != pnet::packet::ethernet::EtherTypes::Ipv4 {
            return None;
        }

        // Parse the IPv4 packet
        let ipv4 = Ipv4Packet::new(eth.payload())?;
        if ipv4.get_next_level_protocol() != IpNextHeaderProtocols::Tcp {
            return None;
        }

        // Parse the TCP segment
        let tcp = TcpPacket::new(ipv4.payload())?;

        Some(ParsedPacket {
            src_ip: ipv4.get_source(),
            dst_ip: ipv4.get_destination(),
            src_port: tcp.get_source(),
            dst_port: tcp.get_destination(),
            sequence: tcp.get_sequence(),
            acknowledgment: tcp.get_acknowledgement(),
            flags: tcp.get_flags(),
            total_length,
            timestamp: packet.header.ts,
        })
    }
}
