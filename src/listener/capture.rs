use log::{error, info};
use mac_address::{get_mac_address, MacAddress};
use pcap::{Capture, Device, Inactive, Packet, PacketHeader};
use pnet::datalink::MacAddr;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tokio::task;
use anyhow::Result;

use crate::*;

pub struct PacketCapturer {
    cap: Capture<Inactive>,
    sender: CapEventSender,
}

pub struct PCAPMeta {
    pub mac_addr: MacAddr,
    pub ipv4: Ipv4Addr,
    pub ipv6: Ipv6Addr,
    pub name: String,
}

impl PCAPMeta {
    pub fn new(device: Device, mac_addr: MacAddress) -> Self {
        let mut ipv4 = Ipv4Addr::new(0, 0, 0, 0);
        let mut ipv6 = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0);
        for addr in &device.addresses {
            match addr.addr {
                IpAddr::V4(ip) => ipv4 = ip,
                IpAddr::V6(ip) => ipv6 = ip,
            }
        }
        PCAPMeta {
            mac_addr: MacAddr::from(mac_addr.bytes()),
            ipv4,
            ipv6,
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
            IpAddr::V4(_) => Some(IpAddr::V4(self.ipv4)),
            IpAddr::V6(_) => Some(IpAddr::V6(self.ipv6)),
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

#[derive(Clone, Debug)]
pub struct OwnedPacket {
    pub header: PacketHeader,
    pub data: Vec<u8>,
}

impl<'a> From<Packet<'a>> for OwnedPacket {
    fn from(packet: Packet<'a>) -> Self {
        OwnedPacket {
            header: *packet.header,
            data: packet.data.to_vec(),
        }
    }
}

impl PacketCapturer {
    /**
     *  Create a new PacketCapturer instance
     */
    pub fn new(sender: CapEventSender) -> CaptureResult {
        // ! Change this to select device by name maybe?
        let device = Device::lookup()?.ok_or("No device available for capture")?;
        info!("Using device: {}", device.name);

        let cap = Capture::from_device(device.clone())?
            .promisc(Settings::PROMISC)
            .immediate_mode(Settings::IMMEDIATE_MODE)
            .timeout(Settings::TIMEOUT) // Timeout in milliseconds
            .tstamp_type(Settings::TSTAMP_TYPE)
            .precision(Settings::PRECISION)
            .snaplen(Settings::SNAPLEN);

        let mac_addr = match get_mac_address() {
            Ok(Some(mac)) => mac,
            Ok(None) => return Err("No MAC address found".into()),
            Err(e) => return Err(e.into()),
        };

        let meta = PCAPMeta::new(device.clone(), mac_addr);

        Ok((
            PacketCapturer {
                cap,
                sender,
            },
            meta,
        ))
    }

    /// Start the asynchronous packet capturing loop
    ///
    /// The idea: Don't block the main thread with packet capture
    /// This way the reciever can be temporarily overloaded without
    /// affecting the packet capture
    ///
    /// Issue: Might cause high Memory and CPU usage
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
