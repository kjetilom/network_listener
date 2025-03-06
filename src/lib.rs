use listener::capture::{OwnedPacket, PCAPMeta, PacketCapturer};
use probe::iperf_json::IperfResponse;
use prost_net::bandwidth_server::PbfMsg;
use surge_ping::SurgeError;
use std::error::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub mod listener;
pub mod logging;
pub mod probe;
pub mod prost_net;
pub mod scheduler;
pub mod config;

pub use listener::packet::*;
pub use listener::tracking::*;
pub use prost_net::bandwidth_client::ClientEvent;
pub use probe::iperf_json::Stream2 as IperfStream;
pub use config::AppConfig;

pub const IPERF3_PORT: u16 = 5201;

pub type CapEventSender = UnboundedSender<CapEvent>;
pub type CapEventReceiver = UnboundedReceiver<CapEvent>;
pub type CaptureResult = Result<(PacketCapturer, PCAPMeta), Box<dyn Error>>;

pub mod proto_bw {
    tonic::include_proto!("bandwidth"); // Matches the package name in .proto
}

use tokio::time::Duration;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref CONFIG: AppConfig = config::load_config();
}

pub struct Settings {}

impl Settings {
    pub const PROMISC: bool = true;
    pub const IMMEDIATE_MODE: bool = true;
    pub const TIMEOUT: i32 = 0;
    pub const TSTAMP_TYPE: pcap::TimestampType = pcap::TimestampType::Adapter;
    pub const PRECISION: pcap::Precision = pcap::Precision::Micro;
    pub const TCP_STREAM_TIMEOUT: Duration = Duration::from_secs(5); //from_secs(900);
    pub const CLEANUP_INTERVAL: Duration = Duration::from_secs(15);
    pub const LONGER_INTERVAL: Duration = Duration::from_secs(20);
    pub const SCHEDULER_DEST: &str = "172.16.0.254:50041";
    pub const BW_SERVER_PORT: u16 = 40042;
    pub const NEAREST_LINK_PHY_CAP: f64 = 5000000.0; // 1250000.0 bytes/sec
    pub const TCPHDR: i32 = 60;
    pub const IPHDR: i32 = 60;
    pub const IPV6HDR: i32 = 40;
    pub const ETHDR: i32 = 14;
    // TCP header without options: 20 bytes
    pub const TCPHDR_NOOPT: i32 = 20;
    pub const SNAPLEN_NOOPT: i32 = Self::TCPHDR_NOOPT + Self::ETHDR + Self::IPHDR; // Max header size=94 bytes.
    pub const SNAPLEN: i32 = Self::TCPHDR + Self::ETHDR + Self::IPHDR; // Max header size=134 bytes.

    // Bandwidth estimation window in seconds. Determines the time window before the data is invalidated.
    // The window should be large enough to gather nessesary data, but small enough to adapt to changing network conditions.
    pub const BWE_WINDOW: i32 = 15;
}

pub enum CapEvent {
    Packet(OwnedPacket),
    IperfResponse(IperfResponse),
    Protobuf(PbfMsg),
    PathloadResponse(String),
    PingResponse(Result<Duration, SurgeError>),
}
