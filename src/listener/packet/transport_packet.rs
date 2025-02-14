use pnet::packet::{
    ip::{IpNextHeaderProtocol, IpNextHeaderProtocols},
    tcp::{TcpOptionIterable, TcpOptionNumbers, TcpPacket},
    udp::UdpPacket,
    Packet,
};

#[derive(Debug)]
pub enum TransportPacket {
    TCP {
        sequence: u32,
        acknowledgment: u32,
        flags: TcpFlags,
        // Maximum size of an IP packet is 65,535 bytes (2^16 - 1)
        payload_len: u16,
        // TCP options (timestamps, window scale)
        options: TcpOptions,
        src_port: u16,
        dst_port: u16,
        window_size: u16,
    },
    UDP {
        src_port: u16,
        dst_port: u16,
        payload_len: u16,
    },
    ICMP,
    OTHER {
        protocol: u8,
    },
}

impl TransportPacket {
    pub fn get_ip_proto(&self) -> IpNextHeaderProtocol {
        match self {
            TransportPacket::TCP { .. } => IpNextHeaderProtocols::Tcp,
            TransportPacket::UDP { .. } => IpNextHeaderProtocols::Udp,
            TransportPacket::ICMP => IpNextHeaderProtocols::Icmp,
            TransportPacket::OTHER { protocol } => IpNextHeaderProtocol(*protocol),
        }
    }

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
                // total size - header size

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

#[derive(Debug)]
pub struct TcpFlags(u8);

impl TcpFlags {
    pub fn new(flags: u8) -> TcpFlags {
        TcpFlags(flags)
    }

    pub const SYN: u8 = 0x02;
    pub const ACK: u8 = 0x10;
    pub const FIN: u8 = 0x01;
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

#[derive(Debug)]
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

    /// Parse TCP options from a TCP packet
    /// Returns a `TcpOptions` struct
    /// # Arguments
    /// * `tcp_options` - An iterator over the TCP options
    pub fn from_bytes(tcp_options: TcpOptionIterable) -> Self {
        let mut options = TcpOptions::new();
        for option in tcp_options {
            match option.get_number() {
                TcpOptionNumbers::TIMESTAMPS => {
                    let timestamp_bytes = option.payload();
                    if timestamp_bytes.len() != 8 {
                        log::warn!(
                            "Invalid timestamp length: expected 8, got {}",
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
