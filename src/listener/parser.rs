use super::packet::packet_builder::ParsedPacket;
use super::{
    analyzer::Analyzer,
    procfs_reader::{self, get_interface, get_interface_info, NetStat},
    stream_manager::StreamManager,
};
use anyhow::Result;
use capture::OwnedPacket;
use log::{error, info};
use neli_wifi::{Bss, Station};
use tokio::{
    sync::mpsc::{self, Receiver, Sender, UnboundedReceiver},
    time,
};

const CHANNEL_CAPACITY: usize = 1000;

use super::capture::{self, PCAPMeta};

#[derive(Debug)]
pub struct NetlinkData {
    pub stations: Vec<Station>, // Currently connected stations
    pub bss: Vec<Bss>,          // BSS information
}

pub struct PeriodicData {
    pub netlink_data: NetlinkData,
    pub netstat_data: NetStat,
}

pub struct Parser {
    packet_stream: UnboundedReceiver<OwnedPacket>,
    pcap_meta: PCAPMeta,
    stream_manager: StreamManager,
    netlink_data: Vec<NetlinkData>,
    netstat_data: Option<NetStat>,
    analyzer: Analyzer,
}

impl Parser {
    pub fn new(
        packet_stream: UnboundedReceiver<OwnedPacket>,
        // "Metadata" from the pcap capture, aka this devices MAC and IP addresses
        pcap_meta: PCAPMeta,
    ) -> Result<Self> {
        Ok(Parser {
            packet_stream,
            pcap_meta,
            stream_manager: StreamManager::default(),
            netlink_data: Vec::new(),
            netstat_data: None,
            analyzer: Analyzer::new(),
        })
    }

    pub async fn start(mut self) {
        let interface = match get_interface(&self.pcap_meta.name).await {
            Ok(interface) => {
                info!("Interface: {:?}", interface);
                interface
            }
            Err(e) => {
                error!("Error getting interface: {:?}", e);
                return;
            }
        };
        let idx = interface.index.unwrap();

        let (ptx, mut prx): (Sender<PeriodicData>, Receiver<PeriodicData>) =
            mpsc::channel(CHANNEL_CAPACITY);

        let periodic_handle = tokio::spawn(async move {
            Parser::periodic(ptx, idx).await;
        });

        let mut interval = time::interval(super::Settings::CLEANUP_INTERVAL);

        loop {
            tokio::select! {
                Some(packet) = self.packet_stream.recv() => {
                    self.handle_capture(packet);
                },
                Some(periodic_data) = prx.recv() => {
                    self.handle_periodic(periodic_data);
                },
                _ = interval.tick() => {
                    self.stream_manager.periodic(self.netstat_data.take());
                },
                else => {
                    // Both streams have ended
                    self.stop(vec![periodic_handle]).await;
                    break;
                }
            }
        }
    }

    pub async fn stop(self, handles: Vec<tokio::task::JoinHandle<()>>) {
        // Stop the parser
        for handle in handles {
            handle.abort();
        }
    }

    async fn periodic(tx: Sender<PeriodicData>, idx: i32) {
        loop {
            let netstat = procfs_reader::proc_net().await;
            let interface = get_interface_info(idx).await.unwrap();

            let data = PeriodicData {
                netlink_data: interface,
                netstat_data: netstat,
            };

            if tx.send(data).await.is_err() {
                break;
            }

            time::sleep(super::Settings::CLEANUP_INTERVAL).await;
        }
    }

    fn handle_periodic(&mut self, data: PeriodicData) {
        self.netlink_data.push(data.netlink_data);
        if self.netlink_data.len() > 10 {
            self.netlink_data.remove(0);
        }

        self.netstat_data = Some(data.netstat_data);
    }

    fn handle_capture(&mut self, packet: OwnedPacket) {
        // Handle the captured packet
        self.analyzer.process_packet(&packet);

        let parsed_packet = match self.parse_packet(packet) {
            Some(packet) => packet,
            None => return,
        };

        self.stream_manager.record_ip_packet(&parsed_packet, &self.pcap_meta);
    }

    /* Parses an `OwnedPacket` into a `ParsedPacket`.
     * Returns `Some(ParsedPacket)` if parsing is successful, otherwise `None`.
     */
    pub fn parse_packet(&self, packet: OwnedPacket) -> Option<ParsedPacket> {
        ParsedPacket::from_packet(&packet, &self.pcap_meta)
    }
}
