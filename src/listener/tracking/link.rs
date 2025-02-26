use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    net::{AddrParseError, IpAddr},
    sync::Arc,
};

use crate::proto_bw::{
    data_msg, BandwidthMessage, DataMsg, LinkState as LinkStateProto, RttMessage, Rtts,
};

use log::{info, warn};
use tokio::sync::mpsc::Sender;

use crate::{
    listener::{packet::ParsedPacket, tracking::stream_manager::StreamManager},
    prost_net::bandwidth_client::ClientHandlerEvent,
};

use super::stream_id::IpPair;
use crate::PCAPMeta;

type Streams = HashMap<IpPair, StreamManager>;

#[derive(Debug)]
pub struct LinkManager {
    links: Streams,             // Private field
    vip_links: HashSet<IpPair>, // Links we care about (Empty at startup)
    client_sender: Sender<ClientHandlerEvent>,
    pcap_meta: Arc<PCAPMeta>,
}

impl LinkManager {
    pub fn new(client_sender: Sender<ClientHandlerEvent>, pcap_meta: Arc<PCAPMeta>) -> Self {
        LinkManager {
            links: HashMap::new(),
            vip_links: HashSet::new(),
            client_sender,
            pcap_meta,
        }
    }

    /// Tries to construct a key from an existing external IP addr.
    /// If the key exists, returns the link.
    pub fn get_link_by_ext_ip(&self, ext_ip: IpAddr) -> Option<&StreamManager> {
        let ip_pair = match ext_ip {
            IpAddr::V4(_) => IpPair::new(ext_ip, self.pcap_meta.ipv4.into()),
            IpAddr::V6(_) => IpPair::new(ext_ip, self.pcap_meta.ipv6.into()),
        };
        self.links.get(&ip_pair)
    }

    pub fn insert(&mut self, packet: ParsedPacket) {
        // Ignore if loopback
        if packet.src_ip.is_loopback() || packet.dst_ip.is_loopback() {
            return;
        }
        if packet.src_ip.is_multicast() || packet.dst_ip.is_multicast() {
            return;
        }
        let ip_pair = IpPair::from_packet(&packet);

        self.links
            .entry(ip_pair)
            .or_insert_with(StreamManager::default)
            .record_packet(&packet);
    }

    pub fn insert_iperf_result(
        &mut self,
        ip_pair: IpPair,
        bps: f64,
        stream: Option<&crate::IperfStream>,
    ) {
        self.links
            .entry(ip_pair)
            .or_insert_with(StreamManager::default)
            .record_iperf_result(bps, stream);
    }

    pub async fn periodic(&mut self) {
        println!();
        for (_, stream_manager) in self.links.iter_mut() {
            stream_manager.periodic();
        }
        for link in self.get_link_states() {
            println!("{}", link);
        }
        self.do_something_with_vip_links().await;
    }

    pub async fn do_something_with_vip_links(&self) {
        for link in self.vip_links.iter() {
            if let Some(stream) = self.links.get(link) {
                if let Some(last_iperf) = stream.last_iperf {
                    if last_iperf.elapsed().as_secs() < 10 {
                        continue;
                    }
                }
            }
            // let ip = link.remote();
            // self.client_sender
            //     .send(ClientHandlerEvent::DoIperf3(
            //         ip.to_string(),
            //         crate::IPERF3_PORT,
            //         1,
            //     ))
            //     .await
            //     .unwrap();

            // Do pathload test (Disabled for now)
            //self.client_sender.send(ClientHandlerEvent::DoPathloadTest(ip.to_string())).await.unwrap();
        }
    }

    pub fn add_important_link(&mut self, ip_addr: Result<IpAddr, AddrParseError>) {
        if let Ok(ip_addr) = ip_addr {
            self.vip_links
                .insert(IpPair::new(self.pcap_meta.ipv4.into(), ip_addr));
        } else {
            info!("Failed to parse IP address");
        }
    }

    pub async fn send_bandwidth(&mut self) {
        let bw_message = self.get_bw_message();
        self.client_sender
            .send(ClientHandlerEvent::SendBandwidth(bw_message))
            .await
            .unwrap_or(warn!("Failed to send bandwidth message"));

        let rtt_message = self.get_rtt_message();
         // ! FIXMELATER

        match self.client_sender
            .send(ClientHandlerEvent::SendBandwidth(rtt_message))
            .await {
                Ok(_) => (),
                Err(e) => warn!("Failed to send rtt message: {}", e),
            }

    }

    pub fn collect_external_ips(&self) -> Vec<IpAddr> {
        self.links.keys().map(|ip_pair| ip_pair.remote()).collect()
    }

    pub async fn send_init_clients_msg(&mut self) {
        self.client_sender
            .send(ClientHandlerEvent::InitClients {
                ips: self.collect_external_ips(),
            })
            .await
            .unwrap();
    }

    pub fn get_bw_message(&mut self) -> DataMsg {
        let links = self.get_link_states();
        DataMsg {
            data: Some(data_msg::Data::Bandwidth(BandwidthMessage {
                link_state: links.into_iter().map(|link| link.to_proto()).collect(),
            })),
        }
    }

    pub fn get_rtt_message(&mut self) -> DataMsg {
        let mut messages: Vec<RttMessage> = self.links.iter_mut()
            .map(|(ip_pair, stream_manager)| {
                RttMessage {
                    sender_ip: ip_pair.local().to_string(),
                    receiver_ip: ip_pair.remote().to_string(),
                    rtt: stream_manager
                    .drain_rtts()
                    .into_iter()
                    .map(|rtt| rtt.to_proto_rtt())
                    .collect(),
                }
            }).collect();
        DataMsg {
            data: Some(data_msg::Data::Rtts(Rtts { rtts: messages })),
        }
    }

    pub fn get_link_states(&mut self) -> Vec<Link> {
        self.links
            .iter_mut()
            .map(|(ip_pair, stream_manager)| {
                let state = LinkState {
                    thp_in: 0.0,
                    thp_out: 0.0,
                    bw: Some(stream_manager.tcp_thput()),
                    abw: Some(stream_manager.abw()),
                    latency: stream_manager.get_latency_avg(),
                    delay: None,
                    jitter: None,
                    loss: None,
                };
                Link {
                    ip_pair: *ip_pair, // Copy IpPair
                    state,
                }
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct LinkState {
    thp_in: f64,          // Throughput in (Measured)
    thp_out: f64,         // Throughput out (Measured)
    bw: Option<f64>,      // bps, None if not available (Bandwidth, estimated)
    abw: Option<f64>,     // bps, None if not available (Available bandwidth, estimated)
    latency: Option<f64>, // ms rtt, None if not available (Measured)
    delay: Option<f64>,   // ms, None if not available (Estimated)
    jitter: Option<f64>,  // ms, None if not available (Measured)
    loss: Option<f64>,    // %, None if not available (Measured)
}

impl LinkState {
    pub fn to_proto(&self, sender_ip: String, receiver_ip: String) -> LinkStateProto {
        LinkStateProto {
            sender_ip: sender_ip,
            receiver_ip: receiver_ip,
            thp_in: self.thp_in,
            thp_out: self.thp_out,
            bw: self.bw.unwrap_or(0.0),
            abw: self.abw.unwrap_or(0.0),
            latency: self.latency.unwrap_or(0.0),
            delay: self.delay.unwrap_or(0.0),
            jitter: self.jitter.unwrap_or(0.0),
            loss: self.loss.unwrap_or(0.0),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

#[derive(Debug)]
pub struct Link {
    ip_pair: IpPair,
    state: LinkState,
}

impl Link {
    pub fn to_proto(&self) -> LinkStateProto {
        // !FIXME THIS IS BAD CHANGE SENDER/RECERIVER TO LOCAL/REMOTE
        self.state.to_proto(
            self.ip_pair.local().to_string(),
            self.ip_pair.remote().to_string(),
        )
    }
}

impl Display for LinkState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "thp_in: {:.2} Kbps, thp_out: {:.2} Kbps, bw: {:?}, abw: {:?}, latency: {:?}, delay: {:?}, jitter: {:?}, loss: {:?}",
            self.thp_in, self.thp_out, self.bw, self.abw, self.latency, self.delay, self.jitter, self.loss)
    }
}

impl Display for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {:?}", self.ip_pair, self.state)
    }
}
