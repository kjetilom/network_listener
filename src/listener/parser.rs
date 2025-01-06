use super::packet::packet_builder::ParsedPacket;
use super::{
    analyzer::Analyzer,
    procfs_reader::{self, get_interface, get_interface_info, NetStat},
    stream_manager::StreamManager,
};
use anyhow::Result;
use capture::OwnedPacket;
use log::{error, info};
use mac_address::MacAddress;
use neli_wifi::{Bss, Interface, Station};
use pcap::Device;
use pnet::util::MacAddr;
use tokio::{
    sync::mpsc::{self, Receiver, Sender, UnboundedReceiver},
    time,
};

const CHANNEL_CAPACITY: usize = 1000;

use super::capture;

#[derive(Debug)]
pub struct NetlinkData {
    pub stations: Vec<Station>, // Currently connected stations
    pub bss: Vec<Bss>,          // BSS information
}

#[derive(Debug)]
pub enum Direction {
    Incoming,
    Outgoing,
}

impl Direction {
    pub fn from_mac(mac: MacAddr, own_mac: MacAddr) -> Self {
        if mac == own_mac {
            Direction::Incoming
        } else {
            Direction::Outgoing
        }
    }

    pub fn is_outgoing(&self) -> bool {
        matches!(self, Direction::Outgoing)
    }
}

pub struct Parser {
    packet_stream: UnboundedReceiver<OwnedPacket>,
    own_mac: MacAddr,
    device_name: String,
    stream_manager: StreamManager,
    netlink_data: Vec<NetlinkData>,
    netstat_data: Option<NetStat>,
    analyzer: Analyzer,
}

impl Parser {
    pub fn new(
        packet_stream: UnboundedReceiver<OwnedPacket>,
        device: Device,
        mac_address: MacAddress,
    ) -> Result<Self> {
        let mac_addr = MacAddr::from(mac_address.bytes());

        Ok(Parser {
            packet_stream,
            own_mac: mac_addr,
            device_name: device.name,
            stream_manager: StreamManager::default(),
            netlink_data: Vec::new(),
            netstat_data: None,
            analyzer: Analyzer::new(),
        })
    }

    pub async fn start(mut self) {
        let interface = match get_interface(&self.device_name).await {
            Ok(interface) => {
                info!("Interface: {:?}", interface);
                interface
            }
            Err(e) => {
                error!("Error getting interface: {:?}", e);
                return;
            }
        };

        // Create bounded channels
        let (netlink_tx, mut netlink_rx): (Sender<NetlinkData>, Receiver<NetlinkData>) =
            mpsc::channel(CHANNEL_CAPACITY);

        let (netstat_tx, mut netstat_rx): (Sender<NetStat>, Receiver<_>) =
            mpsc::channel(CHANNEL_CAPACITY);

        // Spawn netlink_comms and pass the sender
        let netlink_handle = tokio::spawn(async move {
            Parser::netlink_comms(netlink_tx, interface).await;
        });

        let netstat_handle = tokio::spawn(async move {
            Parser::periodic_netstat(netstat_tx).await;
        });

        let mut interval = time::interval(super::Settings::CLEANUP_INTERVAL);

        loop {
            tokio::select! {
                Some(packet) = self.packet_stream.recv() => {
                    self.handle_capture(packet);
                },
                Some(netlink_data) = netlink_rx.recv() => {
                    self.handle_netlink_data(netlink_data);
                },
                Some(netstat_data) = netstat_rx.recv() => {
                    self.handle_netstat_data(netstat_data);
                },
                _ = interval.tick() => {
                    self.stream_manager.periodic(self.netstat_data.take());
                },
                else => {
                    // Both streams have ended
                    break;
                }
            }
        }

        let _ = netlink_handle.await;
        let _ = netstat_handle.await;
    }

    pub async fn stop(self) {
        // Stop the parser
    }

    async fn periodic_netstat(netstat_tx: Sender<NetStat>) {
        loop {
            let netstat = procfs_reader::proc_net().await;

            if netstat_tx.send(netstat).await.is_err() {
                break;
            }

            time::sleep(time::Duration::from_secs(7)).await;
        }
    }

    async fn netlink_comms(netlink_tx: Sender<NetlinkData>, interface: Interface) {
        loop {
            let data = get_interface_info(interface.index.unwrap()).await;

            if netlink_tx.send(data.unwrap()).await.is_err() {
                break;
            }

            time::sleep(time::Duration::from_secs(7)).await;
        }
    }

    /// Placeholder for handling netlink data
    fn handle_netlink_data(&mut self, data: NetlinkData) {
        self.netlink_data.push(data);
        if self.netlink_data.len() > 10 {
            self.netlink_data.remove(0);
        }
    }

    /// Placeholder for handling netstat data
    fn handle_netstat_data(&mut self, data: NetStat) {
        self.netstat_data = Some(data);
    }

    fn handle_capture(&mut self, packet: OwnedPacket) {
        // Handle the captured packet
        self.analyzer.process_packet(&packet);

        let parsed_packet = match self.parse_packet(packet) {
            Some(packet) => packet,
            None => return,
        };

        self.stream_manager.record_ip_packet(&parsed_packet);
    }

    /* Parses an `OwnedPacket` into a `ParsedPacket`.
     * Returns `Some(ParsedPacket)` if parsing is successful, otherwise `None`.
     */
    pub fn parse_packet(&self, packet: OwnedPacket) -> Option<ParsedPacket> {
        ParsedPacket::from_packet(&packet, self.own_mac)
    }
}