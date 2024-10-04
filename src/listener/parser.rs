use pnet::packet::{ethernet::EthernetPacket, ip::IpNextHeaderProtocols, Packet};
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::tcp::TcpPacket;
use pnet::packet::udp::UdpPacket;

pub struct ParsedPacket {
    pub source_ip: String,
    pub destination_ip: String,
    pub source_port: u16,
    pub destination_port: u16,
    pub protocol: String,
    pub payload: Vec<u8>,
}

pub fn parse_packet(packet: pcap::Packet) -> Option<ParsedPacket> {
    let eth = EthernetPacket::new(packet.data)?;
    if eth.get_ethertype() != pnet::packet::ethernet::EtherTypes::Ipv4 {
        return None;
    }

    let ipv4 = Ipv4Packet::new(eth.payload())?;
    let source_ip = ipv4.get_source().to_string();
    let destination_ip = ipv4.get_destination().to_string();

    let protocol = match ipv4.get_next_level_protocol() {
        IpNextHeaderProtocols::Tcp => "TCP",
        IpNextHeaderProtocols::Udp => "UDP",
        _ => return None,
    };

    let source_port;
    let destination_port;

    match protocol {
        "TCP" => {
            let tcp = TcpPacket::new(ipv4.payload())?;
            source_port = tcp.get_source();
            destination_port = tcp.get_destination();
        }
        "UDP" => {
            let udp = UdpPacket::new(ipv4.payload())?;
            source_port = udp.get_source();
            destination_port = udp.get_destination();
        }
        _ => return None,
    }

    Some(ParsedPacket {
        source_ip,
        destination_ip,
        source_port,
        destination_port,
        protocol: protocol.to_string(),
        payload: ipv4.payload().to_vec(),
    })
}