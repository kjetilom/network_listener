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
    pub is_outgoing: bool,
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
            if let Some(duration) = tracker.record_ack(parsed_packet.acknowledgment) {
                println!("RTT: {:?}, Source: {:?}, Destination: {:?}",
                     duration, parsed_packet.src_ip, parsed_packet.dst_ip);
                debug!(
                    "Received ACK for sequence_number: {} (ack_number: {}), RTT = {:?}",
                    parsed_packet.acknowledgment - 1,
                    parsed_packet.acknowledgment,
                    duration
                );
            } else {
                //warn!("No RTT calculated for packet {:?}", parsed_packet);
            }

        }
    }

    /* Parses an `OwnedPacket` into a `ParsedPacket`.
     *
     * Returns `Some(ParsedPacket)` if parsing is successful, otherwise `None`.
     */
    pub fn parse_packet(&self, packet: &OwnedPacket) -> Option<ParsedPacket> {
        // Parse the Ethernet frame
        let total_length = packet.data.len();
        let eth = EthernetPacket::new(&packet.data)?;
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

        // Get source and destination IP and port
        let src_ip = ipv4.get_source();
        let dst_ip = ipv4.get_destination();
        let src_port = tcp.get_source();
        let dst_port = tcp.get_destination();

        // Extract sequence and acknowledgment numbers
        let sequence = tcp.get_sequence();
        let acknowledgment = tcp.get_acknowledgement();

        // Extract TCP flags
        let flags = tcp.get_flags();

        // Optional: Extract payload if needed
        let payload = tcp.payload().to_vec();

        // Determine if the packet is outgoing or incoming
        // This requires knowing your own IP address; for demonstration, we'll compare with a placeholder
        let own_ip = Ipv4Addr::new(192, 168, 1, 100); // Replace with your actual IP
        let is_outgoing = src_ip == own_ip;

        Some(ParsedPacket {
            src_ip,
            dst_ip,
            src_port,
            dst_port,
            sequence,
            acknowledgment,
            flags,
            payload,
            is_outgoing,
            total_length,
        })
    }
}
