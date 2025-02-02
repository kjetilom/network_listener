use std::{collections::{HashMap, HashSet}, fmt::Display, net::IpAddr};

use tokio::sync::mpsc::Sender;

use crate::{listener::{packet::ParsedPacket, tracking::stream_manager::StreamManager}, proto_bw::HelloReply, ClientEvent, Settings};

use super::stream_id::IpPair;

type Streams = HashMap<IpPair, StreamManager>;

#[derive(Debug)]
pub struct LinkManager {
    links: Streams, // Private field
    vip_links: HashSet<IpAddr>, // Links we care about (Empty at startup)
    client_sender: Sender<ClientEvent>,
}

impl LinkManager {
    pub fn new(client_sender: Sender<ClientEvent>) -> Self {
        LinkManager {
            links: HashMap::new(),
            vip_links: HashSet::new(),
            client_sender,
        }
    }

    pub fn insert(&mut self, packet: ParsedPacket) {
        // Ignore if loopback
        if packet.src_ip.is_loopback() || packet.dst_ip.is_loopback() {
            return;
        }
        let ip_pair = IpPair::from_packet(&packet);

        self.links.entry(ip_pair)
            .or_insert_with(StreamManager::default)
            .record_ip_packet(&packet);
    }

    pub fn insert_iperf_result(&mut self, ip_pair: IpPair, bps: f64) {
        self.links.entry(ip_pair)
            .or_insert_with(StreamManager::default)
            .record_iperf_result(bps);
    }

    pub fn periodic(&mut self) {
        println!();
        for (_, stream_manager) in self.links.iter_mut() {
            stream_manager.periodic();
        }
        for link in self.get_link_states() {
            println!("{}", link);
        }
    }

    pub fn do_something_with_vip_links(&self) {
        for link in self.vip_links.iter() {
            println!("VIP Link: {}", link);
        }
    }

    pub fn add_important_link(&mut self, ip_addr: IpAddr) {
        self.vip_links.insert(ip_addr);
    }

    pub async fn init_important_links(&mut self, pcap_meta: &crate::PCAPMeta, sender: Sender<Result<HelloReply, tonic::Status>>) {
        // We want to find out which links are running an instance of network_listener
        // We can do this by sending hello messages to all the links we know about
        // If we get a response, we know that the link is running network_listener
        if self.vip_links.len() != 0 {
            for ip in self.vip_links.iter() {
                let a= self.client_sender.send(ClientEvent::SendHello {
                    ip: ip.to_string(),
                    message: String::from("Hello!"),
                    reply_tx: sender.clone()});
                if a.await.is_err() {
                    eprintln!("Failed to send hello message to {}", ip);
                }
            }
            return;
        }
        for ip_pair in self.links.keys() {
            // Send hello message
            let pair = ip_pair.get_non_matching(pcap_meta.ipv4.into());
            let ips = pair.1.map(|ip| vec![pair.0, ip]).unwrap_or(vec![pair.0]);
            println!("Sending hello messages to: {}", ips.iter().map(|ip| ip.to_string()).collect::<Vec<String>>().join(", "));
            for ip in ips {
                let a= self.client_sender.send(ClientEvent::SendHello {
                    ip: ip.to_string(),
                    message: String::from("Hello!"),
                    reply_tx: sender.clone()});
                if a.await.is_err() {
                    eprintln!("Failed to send hello message to {}", ip);
                }
            }
        }
    }

    pub fn get_link_states(&self) -> Vec<Link> {
        self.links.iter().map(|(ip_pair, stream_manager)| {
            let data_in_out = stream_manager.get_in_out();
            let latency = stream_manager.get_latency_avg();
            //let rt_in_out = stream_manager.get_rt_in_out();
            let in_ = (data_in_out.0 * 8) as f64 / 1000.0 / Settings::CLEANUP_INTERVAL.as_secs_f64(); // INSERT THING HERE
            let out = (data_in_out.1 * 8) as f64 / 1000.0 / Settings::CLEANUP_INTERVAL.as_secs_f64(); // INSERT THING HERE
            let state = LinkState {
                thp_in: in_,
                thp_out: out,
                bw: None, // ! Setting to None for now
                abw: Some(stream_manager.get_abw()),
                latency: latency,
                delay: None,
                jitter: None,
                loss: None,
            };
            Link {
                ip_pair: *ip_pair, // Copy IpPair
                state,
            }
        }).collect()
    }
}

#[derive(Debug)]
pub struct LinkState {
    thp_in: f64, // Throughput in (Measured)
    thp_out: f64, // Throughput out (Measured)
    bw: Option<f64>, // bps, None if not available (Bandwidth, estimated)
    abw: Option<f64>, // bps, None if not available (Available bandwidth, estimated)
    latency: Option<f64>, // ms rtt, None if not available (Measured)
    delay: Option<f64>, // ms, None if not available (Estimated)
    jitter: Option<f64>, // ms, None if not available (Measured)
    loss: Option<f64>, // %, None if not available (Measured)
}

#[derive(Debug)]
pub struct Link {
    ip_pair: IpPair,
    state: LinkState,
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