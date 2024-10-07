use pnet::packet::{ethernet::EthernetPacket, ip::IpNextHeaderProtocols, Packet};
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::tcp::TcpPacket;
use pnet::packet::udp::UdpPacket;
use capture::OwnedPacket;
use log::debug;

use super::capture;

#[derive(Debug)]
pub struct ParsedPacket {
    pub source_ip: String,
    pub destination_ip: String,
    pub source_port: u16,
    pub destination_port: u16,
    pub protocol: String,
    pub payload: Vec<u8>,
}

pub fn parse_packet(packet: OwnedPacket) -> Option<ParsedPacket> {
    // Attempt to parse the Ethernet frame
    let eth = match EthernetPacket::new(&packet.data) {
        Some(pkt) => pkt,
        None => {
            debug!("Failed to parse Ethernet packet.");
            return None;
        }
    };

    if eth.get_ethertype() != pnet::packet::ethernet::EtherTypes::Ipv4 {
        debug!(
            "Non-IPv4 Ethertype: {:?}, skipping packet.",
            eth.get_ethertype()
        );
        return None;
    }

    // Attempt to parse the IPv4 packet
    let ipv4 = match Ipv4Packet::new(eth.payload()) {
        Some(pkt) => pkt,
        None => {
            debug!("Failed to parse IPv4 packet.");
            return None;
        }
    };
    let source_ip = ipv4.get_source().to_string();
    let destination_ip = ipv4.get_destination().to_string();

    // Determine the protocol
    let protocol = match ipv4.get_next_level_protocol() {
        IpNextHeaderProtocols::Tcp => "TCP",
        IpNextHeaderProtocols::Udp => "UDP",
        other => {
            debug!("Unsupported protocol: {:?}", other);
            return None;
        }
    };

    // Extract ports and payload based on protocol
    let (source_port, destination_port, payload) = match protocol {
        "TCP" => {
            let tcp = match TcpPacket::new(ipv4.payload()) {
                Some(pkt) => pkt,
                None => {
                    debug!("Failed to parse TCP packet.");
                    return None;
                }
            };
            (
                tcp.get_source(),
                tcp.get_destination(),
                tcp.payload().to_vec(),
            )
        }
        "UDP" => {
            let udp = match UdpPacket::new(ipv4.payload()) {
                Some(pkt) => pkt,
                None => {
                    debug!("Failed to parse UDP packet.");
                    return None;
                }
            };
            (
                udp.get_source(),
                udp.get_destination(),
                udp.payload().to_vec(),
            )
        }
        _ => return None,
    };

    Some(ParsedPacket {
        source_ip,
        destination_ip,
        source_port,
        destination_port,
        protocol: protocol.to_string(),
        payload,
    })
}
