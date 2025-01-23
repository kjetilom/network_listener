use std::net::IpAddr;
use libc::{ETH_HLEN, IPV6_HDRINCL, IPV6_RECVRTHDR, IPV6_RTHDR, TPACKET_HDRLEN};
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::Packet;
use tokio::time;
use std::time::{SystemTime, UNIX_EPOCH};
use pnet::util::MacAddr;

use crate::listener::capture::{OwnedPacket, PCAPMeta};
use crate::listener::packet::transport_packet::TransportPacket;
use super::direction::{self, Direction};
use pnet::packet::ethernet::{EtherTypes, EthernetPacket};
use pnet::packet::ip::IpNextHeaderProtocol;




/// time::Duration and SystemTime uses Nanosecond precision
pub fn timeval_to_system_time(tv: libc::timeval) -> SystemTime {
    match super::super::Settings::PRECISION {
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

// -----------------------------------
// Zero-copy ParsedPacket
// -----------------------------------
#[derive(Debug)]
pub struct ParsedPacket {
    pub src_ip: IpAddr,
    pub dst_ip: IpAddr,
    pub src_mac: MacAddr,
    pub dst_mac: MacAddr,
    pub transport: TransportPacket,
    pub total_length: u32,
    pub timestamp: SystemTime,
    pub direction: Direction,
    pub intercepted: bool,
}

impl<'a> ParsedPacket {
    /// Convert an OwnedPacket into a borrowed ParsedPacket without copying the payload
    pub fn from_packet(packet: &'a OwnedPacket, pcap_meta: &PCAPMeta) -> Option<ParsedPacket> {
        // Parse Ethernet frame in place
        let eth = EthernetPacket::new(&packet.data)?;
        let total_length = packet.header.len;
        let timestamp = timeval_to_system_time(packet.header.ts);

        // Extract IP info & payload references
        let (src_ip, dst_ip, payload, protocol, hdrlen) = Self::get_ip_info(&eth)?;

        // Build the transport struct from the raw payload reference
        let transport = TransportPacket::from_data(payload, protocol, total_length as u16 - (hdrlen+ETH_HLEN as u16));

        let direction = direction::Direction::from_mac(eth.get_destination(), pcap_meta.mac_addr);

        // The packet is intercepted if A <-> B <-> C and the packet is marked A <-> C
        let intercepted = !pcap_meta.matches_ip(src_ip) && !pcap_meta.matches_ip(dst_ip);


        Some(ParsedPacket {
            src_ip,
            dst_ip,
            src_mac: eth.get_source(),
            dst_mac: eth.get_destination(),
            transport,
            total_length,
            timestamp,
            direction,
            intercepted,
        })
    }

    /// Returns (src_ip, dst_ip, payload, protocol)
    fn get_ip_info(
        eth: &'a EthernetPacket
    ) -> Option<(IpAddr, IpAddr, &'a [u8], IpNextHeaderProtocol, u16)> {
        match eth.get_ethertype() {
            EtherTypes::Ipv4 => Self::parse_ipv4_packet(eth.payload()),
            EtherTypes::Ipv6 => Self::parse_ipv6_packet(eth.payload()),
            _ => None,
        }
    }

    fn parse_ipv4_packet(
        payload: &'a [u8],
    ) -> Option<(IpAddr, IpAddr, &'a [u8], IpNextHeaderProtocol, u16)> {
        let ipv4 = Ipv4Packet::new(payload)?;
        Some((
            IpAddr::V4(ipv4.get_source()),
            IpAddr::V4(ipv4.get_destination()),
            &payload[ipv4.get_header_length() as usize * 4..], // reference to the rest of the IPv4 payload
            ipv4.get_next_level_protocol(),
            ipv4.get_header_length() as u16 * 4,
        ))
    }

    fn parse_ipv6_packet(
        payload: &'a [u8],
    ) -> Option<(IpAddr, IpAddr, &'a [u8], IpNextHeaderProtocol, u16)> {
        let ipv6 = Ipv6Packet::new(payload)?;
        Some((
            IpAddr::V6(ipv6.get_source()),
            IpAddr::V6(ipv6.get_destination()),
            &payload[super::super::Settings::IPV6HDR as usize..], // reference to the rest of the IPv6 payload
            ipv6.get_next_header(),
            40,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;
    use crate::listener::capture::OwnedPacket;
    use pcap::PacketHeader;

    fn create_tcp_packet() -> Vec<u8> {
        // Build a minimal Ethernet+IPv4 header (14 bytes + 20 bytes) + 20-byte TCP header
        let mut packet_data = Vec::with_capacity(14 + 20 + 20);
        // Ethernet header: 6 bytes dst MAC + 6 bytes src MAC + 2 bytes EtherType
        packet_data.extend_from_slice(&[0x00; 6]); // dst MAC
        packet_data.extend_from_slice(&[0x01; 6]); // src MAC
        packet_data.extend_from_slice(&[0x08, 0x00]); // EtherType = IPv4
        // IPv4 header (20 bytes, minimal)
        let ipv4_header = [0x45, 0x00, 0x00, 0x00, // version, IHL=5, DSCP, ECN
                           0x00, 0x00, 0b11100000, 0x00, // total length (will ignore), id
                           0x40, 0x06, 0x00, 0x00, // flags, ttl=64, protocol=TCP
                           0x7F, 0x00, 0x00, 0x01, // src IP
                           0x7F, 0x00, 0x00, 0x02];// dst IP
        packet_data.extend_from_slice(&ipv4_header);
        // TCP header (20 bytes, minimal)
        let tcp_header = [0x00, 0x50, 0x00, 0x50, // src port 80, dst port 80
                          0x00, 0x00, 0x00, 0x00, // seq num
                          0x00, 0x00, 0x00, 0x00, // ack num
                          0x50, 0x02, 0xFF, 0xFF, // data offset, flags, window size
                          0x00, 0x00, 0x00, 0x00];// checksum, urgent pointer
        packet_data.extend_from_slice(&tcp_header);
        packet_data
    }

    #[test]
    fn test_payload_size_1000_removed() {
        // Build a minimal Ethernet+IPv4 header (14 bytes + 20 bytes) + 1000-byte payload
        let mut packet_data = Vec::with_capacity(14 + 20 + 1000);
        // Ethernet header: 6 bytes dst MAC + 6 bytes src MAC + 2 bytes EtherType
        packet_data.extend_from_slice(&[0x00; 6]); // dst MAC
        packet_data.extend_from_slice(&[0x01; 6]); // src MAC
        packet_data.extend_from_slice(&[0x08, 0x00]); // EtherType = IPv4
        // IPv4 header (20 bytes, minimal)
        let mut ipv4_header = [0x45, 0x00, 0x00, 0x00, // version, IHL=5, DSCP, ECN
                               0x00, 0x00, 0x00, 0x00, // total length (will ignore), id
                               0x40, 0x00, 0x40, 0x06, // flags, ttl=64, protocol=TCP
                               0x7F, 0x00, 0x00, 0x01, // src IP 127.0.0.1
                               0x7F, 0x00, 0x00, 0x02];// dst IP 127.0.0.2
        // Adjust the total length field (bytes 2..4) to 20 + 1000
        let total_len = 20 + 1000;
        ipv4_header[2] = (total_len >> 8) as u8;
        ipv4_header[3] = total_len as u8;
        packet_data.extend_from_slice(&ipv4_header);
        // 1000 bytes of payload
        packet_data.extend_from_slice(&vec![0xAB; 1000]);

        let owned_packet = OwnedPacket {
            header: PacketHeader {
                ts: libc::timeval { tv_sec: 0, tv_usec: 0 },
                caplen: (14 + total_len) as u32,
                len: (14 + total_len) as u32,
            },
            data: packet_data.clone(),
        };

        // Parse once with payload
        let pcap_meta = crate::listener::capture::PCAPMeta {
            mac_addr: MacAddr::new(0, 0, 0, 0, 0, 0),
            ipv4: Ipv4Addr::new(0, 0, 0, 0),
            ipv6: Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0),
            name: "test".to_string(),
        };
        let parsed = ParsedPacket::from_packet(&owned_packet, &pcap_meta).unwrap();
        assert_eq!(parsed.total_length, 14 + 20 + 1000);

        // Create the same packet, say its the same size, but remove the payload
        let owned_packet = OwnedPacket {
            header: PacketHeader {
                ts: libc::timeval { tv_sec: 0, tv_usec: 0 },
                caplen: (14 + 20) as u32,
                len: (14 + 20 + 1000) as u32,
            },
            data: packet_data[..14 + 20].to_vec(),
        };

        // Parse again without payload
        let parsed = ParsedPacket::from_packet(&owned_packet, &pcap_meta).unwrap();
        assert_eq!(parsed.total_length, 14 + 20 + 1000);
    }

    #[test]
    fn test_payload_size_1000_removed_tcp() {
        let packet_data = create_tcp_packet();
        let owned_packet = OwnedPacket {
            header: PacketHeader {
                ts: libc::timeval { tv_sec: 0, tv_usec: 0 },
                caplen: packet_data.len() as u32,
                len: packet_data.len() as u32 + 1000, // pretend there's more data
            },
            data: packet_data,
        };

        let pcap_meta = crate::listener::capture::PCAPMeta {
            mac_addr: MacAddr::new(0, 0, 0, 0, 0, 0),
            ipv4: Ipv4Addr::new(0, 0, 0, 0),
            ipv6: Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0),
            name: "test".to_string(),
        };

        let parsed = ParsedPacket::from_packet(&owned_packet, &pcap_meta).unwrap();
        assert_eq!(parsed.total_length, 14 + 20 + 20 + 1000);
        if let TransportPacket::TCP { payload_len, .. } = parsed.transport {
            assert_eq!(payload_len, 1000);
        } else {
            panic!("Expected TCP packet");
        }
    }

}