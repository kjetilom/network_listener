use anyhow::Result;
use log::{error, info};
use mac_address::{get_mac_address, MacAddress};
use pcap::{Capture, Device, Inactive, Packet, PacketHeader};
use pnet::datalink::MacAddr;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tokio::task;

use crate::*;

pub struct PacketCapturer {
    cap: Capture<Inactive>,
    sender: CapEventSender,
}

#[derive(Clone, Debug)]
pub struct PCAPMeta {
    pub mac_addr: MacAddr,
    pub ipv4: Ipv4Addr,
    pub ipv6: Ipv6Addr,
    pub name: String,
}

impl PCAPMeta {
    pub fn new(device: Device, mac_addr: MacAddress) -> Self {
        let mut ipv4 = None;
        let mut ipv6 = None;
        for addr in &device.addresses {
            match addr.addr {
                IpAddr::V4(ip) if ipv4.is_none() => ipv4 = Some(ip),
                IpAddr::V6(ip) if ipv6.is_none() => ipv6 = Some(ip),
                _ => (),
            }
            if ipv4.is_some() && ipv6.is_some() {
                break;
            }
        }
        PCAPMeta {
            mac_addr: MacAddr::from(mac_addr.bytes()),
            ipv4: ipv4.unwrap_or(Ipv4Addr::UNSPECIFIED),
            ipv6: ipv6.unwrap_or(Ipv6Addr::UNSPECIFIED),
            name: device.name.clone(),
        }
    }

    pub fn matches_ip(&self, ip_addr: IpAddr) -> bool {
        match ip_addr {
            IpAddr::V4(ip) => ip == self.ipv4,
            IpAddr::V6(ip) => ip == self.ipv6,
        }
    }

    pub fn get_match(&self, ip_addr: IpAddr) -> Option<IpAddr> {
        match ip_addr {
            IpAddr::V4(_) if self.ipv4 != Ipv4Addr::UNSPECIFIED => Some(IpAddr::V4(self.ipv4)),
            IpAddr::V6(_) if self.ipv6 != Ipv6Addr::UNSPECIFIED => Some(IpAddr::V6(self.ipv6)),
            _ => None,
        }
    }

    pub fn matches(&self, mac_addr: MacAddr, ip_addr: Option<IpAddr>) -> bool {
        if mac_addr == self.mac_addr {
            if let Some(ip) = ip_addr {
                match ip {
                    IpAddr::V4(ip) => ip == self.ipv4,
                    IpAddr::V6(ip) => ip == self.ipv6,
                }
            } else {
                true
            }
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub struct OwnedPacket {
    pub header: PacketHeader,
    pub data: Vec<u8>,
}

impl<'a> From<Packet<'a>> for OwnedPacket {
    fn from(packet: Packet<'a>) -> Self {
        OwnedPacket {
            header: packet.header.to_owned(),
            data: packet.data.to_vec(),
        }
    }
}

impl PacketCapturer {
    /**
     *  Create a new PacketCapturer instance
     */
    pub fn device_by_name(name: &str) -> Result<Device> {
        let device = Device::list()?.into_iter().find(|d| d.name == name);
        match device {
            Some(d) => Ok(d),
            None => Err(anyhow::anyhow!("No device found with name: {}", name)),
        }
    }

    pub fn new(sender: CapEventSender, name: Option<String>) -> CaptureResult {
        let device = match name {
            Some(name) => Self::device_by_name(&name)?,
            None => Device::lookup()?.ok_or("No device available for capture")?,
        };

        info!("Using device: {}", device.name);

        let cap = Capture::from_device(device.clone())?
            .promisc(Settings::PROMISC)
            .immediate_mode(Settings::IMMEDIATE_MODE)
            .timeout(Settings::TIMEOUT) // Timeout in milliseconds
            .tstamp_type(CONFIG.client.tstamp_type)
            .precision(Settings::PRECISION)
            .snaplen(Settings::SNAPLEN);

        let mac_addr = match get_mac_address() {
            Ok(Some(mac)) => mac,
            Ok(None) => return Err("No MAC address found".into()),
            Err(e) => return Err(e.into()),
        };

        let meta = PCAPMeta::new(device.clone(), mac_addr);

        Ok((PacketCapturer { cap, sender }, meta))
    }

    /// Start the asynchronous packet capturing loop
    ///
    /// The idea: Don't block the main thread with packet capture
    /// This way the reciever can be temporarily overloaded without
    /// affecting the packet capture
    pub fn start_capture_loop(self) -> task::JoinHandle<Result<()>> {
        // Clone the sender to move into the thread
        let sender = self.sender.clone();
        // Capture needs to be in a blocking task since pcap::Capture is blocking
        let handle = task::spawn_blocking(move || {
            let mut cap = match self.cap.open() {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to open capture: {}", e);
                    return Err(e.into());
                }
            }; // Open the capture
            loop {
                match cap.next_packet() {
                    Ok(packet) => {
                        let packet = OwnedPacket::from(packet);
                        match sender.send(CapEvent::Packet(packet)) {
                            Ok(_) => {}
                            Err(e) => {
                                return Err(e.into());
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error capturing packet: {}", e);
                        continue;
                    }
                }
            }
        });
        handle
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc as ch;

    use super::*;
    use std::net::IpAddr;

    #[test]
    fn test_pcap_meta_matches_ip() {
        let meta = PCAPMeta {
            mac_addr: MacAddr::new(0, 0, 0, 0, 0, 0),
            ipv4: Ipv4Addr::new(192, 168, 1, 1),
            ipv6: Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0),
            name: "eth0".to_string(),
        };

        assert!(meta.matches_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(!meta.matches_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2))));
        assert!(!meta.matches_ip(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1))));
    }

    #[test]
    fn test_pcap_meta_matches() {
        let meta = PCAPMeta {
            mac_addr: MacAddr::new(0, 0, 0, 0, 0, 0),
            ipv4: Ipv4Addr::new(192, 168, 1, 1),
            ipv6: Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0),
            name: "eth0".to_string(),
        };

        assert!(meta.matches(MacAddr::new(0, 0, 0, 0, 0, 0), None));
        assert!(meta.matches(MacAddr::new(0, 0, 0, 0, 0, 0), Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)))));
        assert!(!meta.matches(MacAddr::new(0, 0, 0, 0, 0, 1), None));
        assert!(!meta.matches(MacAddr::new(0, 0, 0, 0, 0, 0), Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)))));
        assert!(!meta.matches(MacAddr::new(0, 0, 0, 0, 0, 0), Some(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)))));
    }

    #[test]
    fn test_owned_packet_from_packet() {
        let packet = Packet {
            header: &PacketHeader {
                ts: libc::timeval {
                    tv_sec: 0,
                    tv_usec: 0,
                },
                caplen: 0,
                len: 0,
            },
            data: &[0u8],
        };

        let owned_packet = OwnedPacket::from(packet);

        assert_eq!(owned_packet.header.ts.tv_sec, 0);
        assert_eq!(owned_packet.header.ts.tv_usec, 0);
        assert_eq!(owned_packet.header.caplen, 0);
        assert_eq!(owned_packet.header.len, 0);
        assert_eq!(owned_packet.data, &[0u8]);
    }

    #[test]
    fn test_packet_capturer_new() {
        let (sender, _) = ch::unbounded_channel();
        let result = PacketCapturer::new(sender, None);
        assert!(result.is_ok());
    }
}