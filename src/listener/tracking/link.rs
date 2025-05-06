use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    net::{AddrParseError, IpAddr},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    proto_bw::{
        data_msg, BandwidthMessage, DataMsg, LinkState as LinkStateProto, PgmDp, PgmDps,
        PgmMessage, Rtt, RttMessage, Rtts,
    },
    PacketRegistry,
};

use log::{info, warn};
use tokio::sync::mpsc::Sender;

use crate::{
    listener::{packet::ParsedPacket, tracking::stream_manager::StreamManager},
    prost_net::bandwidth_client::ClientHandlerEvent,
    CONFIG,
};

use super::stream_id::IpPair;
use crate::PCAPMeta;

type Streams = HashMap<IpPair, StreamManager>;

/// Manages multiple IP-pair streams, collects metrics, and sends protobuf messages.
#[derive(Debug)]
pub struct LinkManager {
    /// Active streams keyed by local/remote IP pairs.
    links: Streams,
    /// links of special interest (Naming needs to be changed)
    vip_links: HashSet<IpPair>,
    /// Channel to send events to the bandwidth client handler.
    client_sender: Sender<ClientHandlerEvent>,
    /// Metadata from PCAP (local IPs).
    pcap_meta: Arc<PCAPMeta>,
}

impl LinkManager {
    /// Creates a new LinkManager with the given client sender and device metadata.
    pub fn new(client_sender: Sender<ClientHandlerEvent>, pcap_meta: Arc<PCAPMeta>) -> Self {
        LinkManager {
            links: HashMap::new(),
            vip_links: HashSet::new(),
            client_sender,
            pcap_meta,
        }
    }

    /// Looks up a stream manager by external IP address, if present.
    pub fn get_link_by_ext_ip(&self, ext_ip: IpAddr) -> Option<&StreamManager> {
        let ip_pair = match ext_ip {
            IpAddr::V4(_) => IpPair::new(ext_ip, self.pcap_meta.ipv4.into()),
            IpAddr::V6(_) => IpPair::new(ext_ip, self.pcap_meta.ipv6.into()),
        };
        self.links.get(&ip_pair)
    }

    /// Inserts a parsed packet into the appropriate stream manager.
    ///
    /// Filters out loopback and multicast, and any packet to/from the server port.
    pub fn insert(&mut self, packet: ParsedPacket) {
        // Ignore if loopback
        if packet.src_ip.is_loopback() || packet.dst_ip.is_loopback() {
            return;
        }
        // This is done in the current implementation as a hack to avoid spamming
        // all clients seen with gRPC hello messages.
        if packet.src_ip.is_multicast() || packet.dst_ip.is_multicast() {
            return;
        }

        if let Some((src_port, dst_port)) = packet.get_src_dst_port() {
            if dst_port == crate::CONFIG.server.port || src_port == crate::CONFIG.server.port {
                return;
            }
        }
        let ip_pair = IpPair::from_packet(&packet);

        self.links
            .entry(ip_pair)
            .or_insert_with(StreamManager::default)
            .record_packet(&packet);
    }

    /// Inserts iperf measurement results into the registry for a given stream.
    ///
    /// Proof of concept for future active measurement integration.
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

    /// Used by the parser task to perform periodic tasks.
    /// As for now, this is just a pass-through to the stream managers.
    pub async fn periodic(&mut self) {
        for (_, stream_manager) in self.links.iter_mut() {
            stream_manager.periodic();
        }
    }

    /// Marks a stream as important. Used by the parser task when it receives a
    /// gRPC hello response or message.
    /// This is a temporary solution until we have a better way to handle
    /// this logic.
    pub fn add_important_link(&mut self, ip_addr: Result<IpAddr, AddrParseError>) {
        if let Ok(ip_addr) = ip_addr {
            self.vip_links
                .insert(IpPair::new(self.pcap_meta.ipv4.into(), ip_addr));
        } else {
            info!("Failed to parse IP address");
        }
    }

    /// Sends bandwidth, RTT, and PGM data messages over the client channel.
    ///
    /// The only part of this function that should be used in production is the
    /// `send_bandwidth` function. The rest is for gathering data for analysis.
    ///
    /// TODO: Avoid excessive creation of messages.
    pub async fn send_bandwidth(&mut self) {
        let (bw_message, rtt_message, pgm_dps) = self.build_messages();

        let bw_message = DataMsg {
            data: Some(data_msg::Data::Bandwidth(bw_message)),
        };

        let rtt_message = DataMsg {
            data: Some(data_msg::Data::Rtts(rtt_message)),
        };

        if CONFIG.server.send_link_states {
            match self
                .client_sender
                .send(ClientHandlerEvent::SendDataMsg(bw_message))
                .await
            {
                Ok(_) => (),
                Err(e) => warn!("Failed to send bandwidth message: {}", e),
            }
        }

        if CONFIG.server.send_rtts {
            match self
                .client_sender
                .send(ClientHandlerEvent::SendDataMsg(rtt_message))
                .await
            {
                Ok(_) => (),
                Err(e) => warn!("Failed to send rtt message: {}", e),
            }
        }

        if CONFIG.server.send_pgm_dps {
            match self
                .client_sender
                .send(ClientHandlerEvent::SendDataMsg(DataMsg {
                    data: Some(data_msg::Data::Pgmmsg(pgm_dps)),
                }))
                .await
            {
                Ok(_) => (),
                Err(e) => warn!("Failed to send pgm message: {}", e),
            }
        }
    }

    /// Returns all remote IPs currently tracked.
    pub fn collect_external_ips(&self) -> Vec<IpAddr> {
        self.links.keys().map(|ip_pair| ip_pair.remote()).collect()
    }

    /// Sends initial client registration message with known IPs.
    pub async fn send_init_clients_msg(&mut self) {
        self.client_sender
            .send(ClientHandlerEvent::InitClients {
                ips: self.collect_external_ips(),
            })
            .await
            .unwrap();
    }

    /// Creates an RTT message from a vector of RTTs and an IP pair.
    pub fn get_rtt_message(rtts: Vec<(u32, SystemTime)>, ip_pair: IpPair) -> RttMessage {
        let messages: Vec<Rtt> = rtts
            .into_iter()
            .map(|(rtt, timestamp)| Rtt {
                rtt: rtt as f64,
                // This is bad practice. Safe for now, as timestamps will always be in the past.
                timestamp: timestamp.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64,
            })
            .collect();

        RttMessage {
            sender_ip: ip_pair.local().to_string(),
            receiver_ip: ip_pair.remote().to_string(),
            rtt: messages,
        }
    }

    /// Internal helper to produce LinkState and PGM for one stream.
    fn get_link_state(
        stream_manager: &mut StreamManager,
        pkt_reg: &mut PacketRegistry,
        ip_pair: IpPair,
    ) -> (Link, PgmDps) {
        let (abw, _dps) = pkt_reg.passive_abw(crate::CONFIG.client.regression_type);
        let tstamp = chrono::Utc::now().timestamp_millis();

        let pgm = PgmDps {
            pgm_dp: std::mem::take(&mut pkt_reg.pgm_estimator.dps)
                .into_iter()
                .map(|dp| PgmDp {
                    gin: dp.gin,
                    gout: dp.gout,
                    len: dp.len as i32,
                    num_acked: dp.num_acked as i32,
                })
                .collect(),
            timestamp: tstamp,
            sender_ip: ip_pair.local().to_string(),
            receiver_ip: ip_pair.remote().to_string(),
        };
        let state = LinkState {
            thp_in: stream_manager.take_received() as f64
                / crate::CONFIG.client.measurement_window.as_secs_f64(),
            thp_out: stream_manager.take_sent() as f64
                / crate::CONFIG.client.measurement_window.as_secs_f64(),
            bw: Some(stream_manager.tcp_thput()),
            abw,
            latency: pkt_reg.avg_rtt(),
            delay: None,
            jitter: None,
            loss: None,
            timestamp: tstamp,
        };
        (Link { ip_pair, state }, pgm)
    }

    /// Builds protobuf messages for bandwidth, RTTs, and PGM data.
    pub fn build_messages(&mut self) -> (BandwidthMessage, Rtts, PgmMessage) {
        let mut links = Vec::new();
        let mut rtts = Vec::new();
        let mut pgm_dps = Vec::new();
        for (ip_pair, stream_manager) in self.links.iter_mut() {
            let mut sent_registry = stream_manager.sent.take();
            let _ = stream_manager.received.take();
            let (link, pgm) = Self::get_link_state(stream_manager, &mut sent_registry, *ip_pair);
            let rtt_msg = Self::get_rtt_message(sent_registry.rtts, *ip_pair);
            links.push(link.to_proto());
            rtts.push(rtt_msg);
            pgm_dps.push(pgm);
        }

        (
            BandwidthMessage { link_state: links },
            Rtts { rtts },
            PgmMessage { pgm_dps },
        )
    }
}

/// Represents the measured and estimated state of a link at an instant.
/// Most of the parameters are unused, but kept for future use.
///
/// The ones that are most significant are:
/// - `thp_in`: Measured throughput in Kbps
/// - `thp_out`: Measured throughput out Kbps
/// - `abw`: Estimated available bandwidth in bytes/sec
/// - `latency`: Measured latency in ms (Not an accurate representation of RTT)
#[derive(Debug)]
pub struct LinkState {
    /// Throughput in and out (Measured)
    thp_in: f64,
    /// Throughput out (Measured)
    thp_out: f64,
    /// bps, None if not available (unused)
    bw: Option<f64>,
    /// bps, None if not available (Available bandwidth, estimated)
    abw: Option<f64>,
    /// ms rtt, None if not available (Measured)
    latency: Option<f64>,
    /// ms, None if not available (Estimated, unused)
    delay: Option<f64>,
    /// ms, None if not available (Measured, unused)
    jitter: Option<f64>,
    /// %, None if not available (Measured, unused)
    loss: Option<f64>,
    /// Timestamp of the measurement
    timestamp: i64,
}

impl LinkState {
    /// Converts internal state to protobuf message.
    pub fn to_proto(&self) -> LinkStateProto {
        LinkStateProto {
            sender_ip: String::new(), // filled by caller
            receiver_ip: String::new(),
            thp_in: self.thp_in,
            thp_out: self.thp_out,
            bw: self.bw.unwrap_or(0.0),
            abw: self.abw.unwrap_or(0.0),
            latency: self.latency.unwrap_or(0.0),
            delay: self.delay.unwrap_or(0.0),
            jitter: self.jitter.unwrap_or(0.0),
            loss: self.loss.unwrap_or(0.0),
            timestamp: self.timestamp,
        }
    }
}

#[derive(Debug)]
pub struct Link {
    ip_pair: IpPair,
    state: LinkState,
}

impl Link {
    /// Converts to protobuf, injecting IP strings.
    pub fn to_proto(&self) -> LinkStateProto {
        let mut msg = self.state.to_proto();
        msg.sender_ip = self.ip_pair.local().to_string();
        msg.receiver_ip = self.ip_pair.remote().to_string();
        msg
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn test_linkstate_display_and_proto() {
        let state = LinkState {
            thp_in: 1.0,
            thp_out: 2.0,
            bw: Some(3.0),
            abw: Some(4.0),
            latency: Some(5.0),
            delay: None,
            jitter: None,
            loss: None,
            timestamp: 0,
        };
        let s = format!("{}", state);
        assert!(s.contains("thp_in: 1.00"));
        let proto = state.to_proto();
        assert_eq!(proto.thp_in, 1.0);
    }

    #[test]
    fn test_link_display() {
        let ipl: IpAddr = [192, 168, 1, 1].into();
        let ipr: IpAddr = [10, 0, 0, 1].into();
        let lp = Link {
            ip_pair: IpPair::new(ipl, ipr),
            state: LinkState {
                thp_in: 0.0,
                thp_out: 0.0,
                bw: None,
                abw: None,
                latency: None,
                delay: None,
                jitter: None,
                loss: None,
                timestamp: 0,
            },
        };
        let s = format!("{}", lp);
        assert!(s.contains("192.168.1.1"));
    }
}
