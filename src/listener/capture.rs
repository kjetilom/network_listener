use log::{error, info};
use mac_address::{get_mac_address, MacAddress};
use pnet::datalink::MacAddr;
use pcap::{Active, Capture, Device, Packet, PacketHeader};
use std::error::Error;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task;

use crate::listener::Settings;

pub struct PacketCapturer {
    cap: Capture<Active>,
    sender: UnboundedSender<OwnedPacket>,
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
            ipv4: ipv4,
            ipv6: ipv6,
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
    pub fn new(
    ) -> Result<(Self, UnboundedReceiver<OwnedPacket>, PCAPMeta), Box<dyn Error>> {
        // ! Change this to select device by name maybe?
        let device = Device::lookup()?.ok_or("No device available for capture")?;
        info!("Using device: {}", device.name);

        let cap = Capture::from_device(device.clone())?
            .promisc(Settings::PROMISC)
            .immediate_mode(Settings::IMMEDIATE_MODE)
            .timeout(Settings::TIMEOUT) // Timeout in milliseconds
            .tstamp_type(Settings::TSTAMP_TYPE)
            .precision(Settings::PRECISION)
            .open()?;

        let mac_addr = match get_mac_address() {
            Ok(Some(ma)) => ma,
            Ok(None) => return Err("No MAC address found".into()),
            Err(e) => return Err(e.into()),
        };

        let (sender, receiver) = unbounded_channel();

        let meta = PCAPMeta::new(device.clone(), mac_addr);

        Ok((PacketCapturer { cap, sender }, receiver, meta))
    }

    pub fn monitor_device(
        dev_name: String,
    ) -> Result<(Self, UnboundedReceiver<OwnedPacket>, Device), Box<dyn Error>> {
        // Given that a monitor device exists, we can capture packets here.
        // As of now, this is not used.
        let device = Device::list()?
            .into_iter()
            .find(|d| d.name == dev_name)
            .ok_or("No device available for capture")?;
        info!("Using device: {}", device.name);
        let mut cap = Capture::from_device(device.clone())?
            .promisc(false)
            .immediate_mode(Settings::IMMEDIATE_MODE)
            .timeout(Settings::TIMEOUT) // Timeout in milliseconds
            .tstamp_type(Settings::TSTAMP_TYPE)
            .precision(Settings::PRECISION)
            .open()?;
        cap.set_datalink(pcap::Linktype(127)).unwrap();
        let (sender, receiver) = unbounded_channel();

        Ok((PacketCapturer { cap, sender }, receiver, device))
    }

    /**
     *  Start the asynchronous packet capturing loop
     */
    pub fn start_capture_loop(mut self) -> task::JoinHandle<()> {
        // Clone the sender to move into the thread
        let sender = self.sender.clone();
        // Capture needs to be in a blocking task since pcap::Capture is blocking
        let handle = task::spawn_blocking(move || {
            loop {
                self.cap.stats().unwrap();
                match self.cap.next_packet() {
                    Ok(packet) => {
                        let packet = OwnedPacket::from(packet);
                        if sender.send(packet).is_err() {
                            // Receiver has been dropped
                            error!("Receiver dropped. Stopping packet capture.");
                            break;
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
