use crate::probe::iperf_json::IperfResponse;

use super::procfs_reader::{self, get_interface, get_interface_info, NetStat};
use super::tracking::link::LinkManager;

use crate::{
    stream_id::from_iperf_connected, CapEvent, CapEventReceiver, OwnedPacket, PCAPMeta,
    ParsedPacket, Settings,
};
use anyhow::Result;
use log::{error, info, warn};
use neli_wifi::{Bss, Station};
use pnet::packet::ip::IpNextHeaderProtocols;
use tokio::task::JoinHandle;
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    time,
};

const CHANNEL_CAPACITY: usize = 10; // Amount of messages. Not Bytes.

#[derive(Debug)]
pub struct NetlinkData {
    pub stations: Vec<Station>, // Currently connected stations
    pub bss: Vec<Bss>,          // BSS information
}

pub struct PeriodicData {
    pub netlink_data: Option<NetlinkData>,
    pub netstat_data: NetStat,
}

pub struct Parser {
    packet_stream: CapEventReceiver,
    pcap_meta: PCAPMeta,
    link_manager: LinkManager,
    netlink_data: Vec<NetlinkData>,
    netstat_data: Option<NetStat>,
}

impl Parser {
    pub fn new(
        packet_stream: CapEventReceiver,
        // "Metadata" from the pcap capture, aka this devices MAC and IP addresses
        pcap_meta: PCAPMeta,
    ) -> Result<Self> {
        Ok(Parser {
            packet_stream,
            pcap_meta,
            link_manager: LinkManager::new(),
            netlink_data: Vec::new(),
            netstat_data: None,
        })
    }

    pub fn dispatch_parser(self) -> JoinHandle<()> {
        tokio::spawn(async move { self.start().await })
    }

    pub async fn start(mut self) {
        let interface = match get_interface(&self.pcap_meta.name).await {
            Ok(interface) => {
                info!("Interface: {:?}", interface);
                Some(interface)
            }
            Err(e) => {
                error!("Failed to get interface index: {}", e);
                None
            }
        };

        let idx = match interface {
            Some(interface) => interface.index,
            None => None,
        };

        let (ptx, mut prx): (Sender<PeriodicData>, Receiver<PeriodicData>) =
            channel(CHANNEL_CAPACITY);

        let periodic_handle = tokio::spawn(async move {
            Parser::periodic(ptx, idx).await;
        });

        let mut interval = time::interval(Settings::CLEANUP_INTERVAL);
        loop {
            tokio::select! {
                Some(cap_ev) = self.packet_stream.recv() => {
                    match cap_ev {
                        CapEvent::Packet(packet) => {
                            self.handle_capture(packet);
                        }
                        CapEvent::IperfResponse(data) => {
                            self.handle_iperf(data);
                        }
                        CapEvent::Protobuf(pbf) => {
                            info!("Received protobuf: {:?}", pbf);
                        }
                    }
                },
                Some(periodic_data) = prx.recv() => {
                    self.handle_periodic(periodic_data);
                },
                _ = interval.tick() => {
                    self.link_manager.periodic();
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

    async fn periodic(tx: Sender<PeriodicData>, idx: Option<i32>) {
        loop {
            let netstat = procfs_reader::proc_net().await;
            let interface = match idx {
                Some(idx) => Some(get_interface_info(idx).await.unwrap()),
                None => None,
            };

            let data = PeriodicData {
                netlink_data: interface,
                netstat_data: netstat,
            };

            if tx.send(data).await.is_err() {
                break;
            }

            time::sleep(Settings::CLEANUP_INTERVAL).await;
        }
    }

    fn handle_periodic(&mut self, data: PeriodicData) {
        match data.netlink_data {
            Some(data) => self.netlink_data.push(data),
            _ => (),
        }
        if self.netlink_data.len() > 10 {
            self.netlink_data.remove(0);
        }

        self.netstat_data = Some(data.netstat_data);
    }

    fn handle_capture(&mut self, packet: OwnedPacket) {
        // Handle the captured packet
        let parsed_packet = match ParsedPacket::from_packet(&packet, &self.pcap_meta) {
            Some(packet) => packet,
            None => return,
        };

        self.link_manager.insert(parsed_packet);
    }

    fn handle_iperf(&mut self, iperf_data: IperfResponse) {
        match iperf_data {
            IperfResponse::Error(e) => {
                warn!("Iperf error: {}", e.error);
            }
            IperfResponse::Success(s) => {
                let connected = s.start.connected;
                if connected.len() == 1 {
                    let (_stream_id, ip_pair) = // ! Use stream_id later pls
                        from_iperf_connected(
                            connected.first().unwrap(),
                            IpNextHeaderProtocols::Tcp
                        );
                    self.link_manager.insert_iperf_result(
                        ip_pair,
                        s.end
                            .sum_received
                            .bits_per_second
                            .max(s.end.sum_sent.bits_per_second),
                    ); // ! This is a hack
                }
            }
        }
    }
}
