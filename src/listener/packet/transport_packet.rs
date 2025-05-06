use pnet::packet::{
    ip::{IpNextHeaderProtocol, IpNextHeaderProtocols},
    tcp::{TcpOptionIterable, TcpOptionNumbers, TcpPacket},
    udp::UdpPacket,
    Packet,
};

/// Represents a transport-layer packet parsed from raw bytes.
///
/// Supports TCP, UDP, ICMP, and other IP protocols.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TransportPacket {
    /// TCP packet with header fields and payload length.
    TCP {
        sequence: u32,
        acknowledgment: u32,
        /// TCP flags struct (Should be changed to a bitfield)
        flags: TcpFlags,
        payload_len: u16,
        options: TcpOptions,
        src_port: u16,
        dst_port: u16,
        window_size: u16,
    },
    /// UDP packet with ports and payload length.
    UDP {
        src_port: u16,
        dst_port: u16,
        payload_len: u16,
    },
    /// ICMP packet (no additional fields).
    ICMP,
    /// Other IP protocol with protocol number.
    /// This is used for protocols not explicitly handled (e.g., GRE, ESP).
    OTHER {
        protocol: u8,
    },
}

impl TransportPacket {
    /// Returns the IP protocol identifier for this packet.
    pub fn get_ip_proto(&self) -> IpNextHeaderProtocol {
        match self {
            TransportPacket::TCP { .. } => IpNextHeaderProtocols::Tcp,
            TransportPacket::UDP { .. } => IpNextHeaderProtocols::Udp,
            TransportPacket::ICMP => IpNextHeaderProtocols::Icmp,
            TransportPacket::OTHER { protocol } => IpNextHeaderProtocol(*protocol),
        }
    }

    /// Parses a transport packet from raw payload bytes, given the IP protocol
    /// and total payload length (including headers).
    ///
    /// Falls back to `OTHER` if parsing fails or protocol unsupported.
    pub fn from_data(payload: &[u8], protocol: IpNextHeaderProtocol, payload_len: u16) -> Self {
        match protocol {
            IpNextHeaderProtocols::Tcp => {
                let tcp = match TcpPacket::new(payload) {
                    Some(tcp) => tcp,
                    None => {
                        log::warn!("Failed to parse TCP packet");
                        return TransportPacket::OTHER {
                            protocol: protocol.0,
                        };
                    }
                };

                let hdr_size = tcp.get_data_offset() as u16 * 4;
                let payload_len = payload_len - hdr_size as u16;

                TransportPacket::TCP {
                    sequence: tcp.get_sequence(),
                    acknowledgment: tcp.get_acknowledgement(),
                    flags: TcpFlags::new(tcp.get_flags()),
                    payload_len,
                    options: TcpOptions::from_bytes(tcp.get_options_iter()),
                    src_port: tcp.get_source(),
                    dst_port: tcp.get_destination(),
                    window_size: tcp.get_window(),
                }
            }
            IpNextHeaderProtocols::Udp => {
                let udp = match UdpPacket::new(payload) {
                    Some(udp) => udp,
                    None => {
                        log::warn!("Failed to parse UDP packet");
                        return TransportPacket::OTHER {
                            protocol: protocol.0,
                        };
                    }
                };
                TransportPacket::UDP {
                    src_port: udp.get_source(),
                    dst_port: udp.get_destination(),
                    payload_len,
                }
            }
            IpNextHeaderProtocols::Icmp => TransportPacket::ICMP,
            _ => TransportPacket::OTHER {
                protocol: protocol.0,
            },
        }
    }
}

/// Wrapper around the TCP control flags byte.
/// Only a partial implementation, as not all flags are used.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct TcpFlags(u8);

impl TcpFlags {
    /// Creates a new `TcpFlags` from a raw flags byte.
    pub fn new(flags: u8) -> TcpFlags {
        TcpFlags(flags)
    }

    /// SYN flag (0x02).
    pub const SYN: u8 = 0x02;
    /// ACK flag (0x10).
    pub const ACK: u8 = 0x10;
    /// FIN flag (0x01).
    pub const FIN: u8 = 0x01;
    /// RST flag (0x04).
    pub const RST: u8 = 0x04;

    pub fn is_syn(&self) -> bool {
        self.0 & Self::SYN != 0
    }
    pub fn is_ack(&self) -> bool {
        self.0 & Self::ACK != 0
    }
    pub fn is_fin(&self) -> bool {
        self.0 & Self::FIN != 0
    }
    pub fn is_rst(&self) -> bool {
        self.0 & Self::RST != 0
    }
}

/// Parsed TCP options of interest: timestamps, window scale, MSS.
/// Only a subset of TCP options is implemented.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TcpOptions {
    pub tsval: Option<u32>,
    pub tsecr: Option<u32>,
    pub scale: Option<u8>,
    pub mss: Option<u16>,
}

impl Default for TcpOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpOptions {
    pub fn new() -> Self {
        TcpOptions {
            tsval: None,
            tsecr: None,
            scale: None,
            mss: None,
        }
    }

    /// Parses options from a TCP packet iterator.
    ///
    /// Recognizes TIMESTAMPS, WSCALE, and MSS; logs and skips invalid lengths.
    pub fn from_bytes(tcp_options: TcpOptionIterable) -> Self {
        let mut options = TcpOptions::new();
        for option in tcp_options {
            match option.get_number() {
                TcpOptionNumbers::TIMESTAMPS => {
                    let timestamp_bytes = option.payload();
                    if timestamp_bytes.len() != 8 {
                        log::warn!(
                            "Invalid TCP TIMESTAMPS length: expected 8, got {}",
                            timestamp_bytes.len()
                        );
                        continue;
                    }
                    options.tsval = Some(u32::from_be_bytes([
                        timestamp_bytes[0],
                        timestamp_bytes[1],
                        timestamp_bytes[2],
                        timestamp_bytes[3],
                    ]));
                    options.tsecr = Some(u32::from_be_bytes([
                        timestamp_bytes[4],
                        timestamp_bytes[5],
                        timestamp_bytes[6],
                        timestamp_bytes[7],
                    ]));
                }
                TcpOptionNumbers::WSCALE => {
                    let scale_bytes = option.payload();
                    if scale_bytes.len() != 1 {
                        log::warn!(
                            "Invalid window scale length: expected 1, got {}",
                            scale_bytes.len()
                        );
                        continue;
                    }
                    options.scale = Some(scale_bytes[0]);
                }
                TcpOptionNumbers::MSS => {
                    let mss_bytes = option.payload();
                    if mss_bytes.len() != 2 {
                        log::warn!("Invalid MSS length: expected 2, got {}", mss_bytes.len());
                        continue;
                    }
                    options.mss = Some(u16::from_be_bytes([mss_bytes[0], mss_bytes[1]]));
                }
                _ => {}
            }
        }
        options
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use pnet::packet::ip::IpNextHeaderProtocols;

    #[test]
    fn test_get_ip_proto_variants() {
        let tcp = TransportPacket::OTHER { protocol: 0 };
        assert_eq!(tcp.get_ip_proto(), IpNextHeaderProtocol(0));
        let udp = TransportPacket::UDP { src_port:1, dst_port:2, payload_len:0 };
        assert_eq!(udp.get_ip_proto(), IpNextHeaderProtocols::Udp);
        let icmp = TransportPacket::ICMP;
        assert_eq!(icmp.get_ip_proto(), IpNextHeaderProtocols::Icmp);
        let tcp_pkt = TransportPacket::TCP { sequence:0, acknowledgment:0, flags:TcpFlags::new(0), payload_len:0, options:TcpOptions::new(), src_port:0, dst_port:0, window_size:0 };
        assert_eq!(tcp_pkt.get_ip_proto(), IpNextHeaderProtocols::Tcp);
    }

    #[test]
    fn test_from_data_udp_success() {
        // 8-byte UDP header: src=80, dst=443, len=8, checksum=0
        let buf = [0x00,0x50, 0x01,0xbb, 0x00,0x08, 0x00,0x00];
        let pkt = TransportPacket::from_data(&buf, IpNextHeaderProtocols::Udp, 8);
        assert_eq!(pkt, TransportPacket::UDP { src_port:80, dst_port:443, payload_len:8 });
    }

    #[test]
    fn test_from_data_udp_fail() {
        let buf = [0u8;4];
        let pkt = TransportPacket::from_data(&buf, IpNextHeaderProtocols::Udp, 4);
        if let TransportPacket::OTHER { protocol } = pkt { assert_eq!(protocol, IpNextHeaderProtocols::Udp.0); } else { panic!("Expected OTHER"); }
    }

    #[test]
    fn test_from_data_tcp_min_header() {
        // TCP header with data_offset=5, flags=ACK
        let mut buf = [0u8;20];
        buf[0..2].copy_from_slice(&80u16.to_be_bytes());
        buf[2..4].copy_from_slice(&443u16.to_be_bytes());
        buf[4..8].copy_from_slice(&1u32.to_be_bytes());
        buf[8..12].copy_from_slice(&2u32.to_be_bytes());
        buf[12] = 5 << 4; // data offset = 5
        buf[13] = TcpFlags::ACK;
        buf[14..16].copy_from_slice(&3u16.to_be_bytes()); // window size
        let pkt = TransportPacket::from_data(&buf, IpNextHeaderProtocols::Tcp, 20);
        if let TransportPacket::TCP { sequence, acknowledgment, flags, payload_len, options, src_port, dst_port, window_size } = pkt {
            assert_eq!(src_port, 80);
            assert_eq!(dst_port, 443);
            assert_eq!(sequence, 1);
            assert_eq!(acknowledgment, 2);
            assert!(flags.is_ack());
            assert_eq!(payload_len, 0);
            assert_eq!(options, TcpOptions::new());
            assert_eq!(window_size, 3);
        } else {
            panic!("Expected TCP variant");
        }
    }

    #[test]
    fn test_from_data_tcp_fail() {
        let buf = [0u8;10];
        let pkt = TransportPacket::from_data(&buf, IpNextHeaderProtocols::Tcp, 10);
        if let TransportPacket::OTHER { protocol } = pkt { assert_eq!(protocol, IpNextHeaderProtocols::Tcp.0); } else { panic!("Expected OTHER"); }
    }

    #[test]
    fn test_tcp_flags_methods() {
        let flags = TcpFlags::new(TcpFlags::SYN | TcpFlags::FIN);
        assert!(flags.is_syn());
        assert!(!flags.is_ack());
        assert!(flags.is_fin());
        assert!(!flags.is_rst());
    }

    #[test]
    fn test_tcp_options_default_and_empty() {
        let default = TcpOptions::new();
        assert_eq!(default, TcpOptions::default());
        // no options set
        let mut buf = [0u8;20];
        buf[12] = 5 << 4;
        let tcp = TcpPacket::new(&buf).unwrap();
        let opts = TcpOptions::from_bytes(tcp.get_options_iter());
        assert_eq!(opts, TcpOptions::new());
    }
}
