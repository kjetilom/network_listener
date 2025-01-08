use std::net::IpAddr;
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
        let (src_ip, dst_ip, payload, protocol) = Self::get_ip_info(&eth)?;

        // Build the transport struct from the raw payload reference
        let transport = TransportPacket::from_data(payload, protocol);

        let direction = direction::Direction::from_mac(eth.get_destination(), pcap_meta.mac_addr);
        // The packet is intercepted if A <-> B <-> C and the packet is marked A <-> C
        let intercepted = !pcap_meta.matches_ip(src_ip) && !pcap_meta.matches_ip(dst_ip);
        // if direction.is_outgoing() && !pcap_meta.matches_ip(src_ip) {
        //     return None;
        // }

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
    ) -> Option<(IpAddr, IpAddr, &'a [u8], IpNextHeaderProtocol)> {
        match eth.get_ethertype() {
            EtherTypes::Ipv4 => Self::parse_ipv4_packet(eth.payload()),
            EtherTypes::Ipv6 => Self::parse_ipv6_packet(eth.payload()),
            _ => None,
        }
    }

    fn parse_ipv4_packet(
        payload: &'a [u8],
    ) -> Option<(IpAddr, IpAddr, &'a [u8], IpNextHeaderProtocol)> {
        let ipv4 = Ipv4Packet::new(payload)?;
        Some((
            IpAddr::V4(ipv4.get_source()),
            IpAddr::V4(ipv4.get_destination()),
            &payload[ipv4.get_header_length() as usize * 4..], // reference to the rest of the IPv4 payload
            ipv4.get_next_level_protocol(),
        ))
    }

    fn parse_ipv6_packet(
        payload: &'a [u8],
    ) -> Option<(IpAddr, IpAddr, &'a [u8], IpNextHeaderProtocol)> {
        let ipv6 = Ipv6Packet::new(payload)?;
        Some((
            IpAddr::V6(ipv6.get_source()),
            IpAddr::V6(ipv6.get_destination()),
            &payload[40..], // reference to the rest of the IPv6 payload
            ipv6.get_next_header(),
        ))
    }
}
