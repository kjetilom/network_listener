use std::net::Ipv4Addr;

use log::{debug, warn};
use pnet::packet::{ethernet::EthernetPacket, ip::IpNextHeaderProtocols, Packet};
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::tcp::{TcpOption, TcpPacket};
use capture::OwnedPacket;
use super::analyzer::Analyzer;
use tokio::sync::mpsc::UnboundedReceiver;
use tracker::PacketTracker;

use super::tracker;
use super::capture;


pub struct Parser {
    packet_stream: UnboundedReceiver<OwnedPacket>,
}

#[derive(Debug)]
pub struct ParsedPacket {
    pub src_ip: Ipv4Addr,
    pub dst_ip: Ipv4Addr,
    pub src_port: u16,
    pub dst_port: u16,
    pub sequence: u32,
    pub acknowledgment: u32,
    pub flags: u8,
    pub payload: Vec<u8>,
    pub total_length: usize,
}

impl Parser {
    pub fn new(packet_stream: UnboundedReceiver<OwnedPacket>) -> Self {
        Parser { packet_stream }
    }

    pub async fn start(mut self) {
        let mut analyzer = Analyzer::new();
        let mut tracker = PacketTracker::new();

        while let Some(packet) = self.packet_stream.recv().await {

            let parsed_packet = match self.parse_packet(&packet) {
                Some(packet) => packet,
                None => continue,
            };
            // Packet has been parsed
            //debug!("{:?}", parsed_packet);
            analyzer.process_packet(&parsed_packet);

            tracker.record_sent(parsed_packet.sequence);
            match tracker.record_ack(parsed_packet.acknowledgment) {
                Some(duration) => {
                    println!("RTT: {:?}, Source: {:?}, Destination: {:?}",
                         duration, parsed_packet.src_ip, parsed_packet.dst_ip);
                }
                None => {},
            }

        }
    }

    /* Parses an `OwnedPacket` into a `ParsedPacket`.
     * Returns `Some(ParsedPacket)` if parsing is successful, otherwise `None`.
     */
    pub fn parse_packet(&self, packet: &OwnedPacket) -> Option<ParsedPacket> {
        // Parse the Ethernet frame
        let total_length = packet.data.len();
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

        Some (ParsedPacket {
            src_ip: ipv4.get_source(),
            dst_ip: ipv4.get_destination(),
            src_port: tcp.get_source(),
            dst_port: tcp.get_destination(),
            sequence: tcp.get_sequence(),
            acknowledgment: tcp.get_acknowledgement(),
            flags: tcp.get_flags(),
            payload: tcp.payload().to_vec(),
            total_length,
        })
    }
}
