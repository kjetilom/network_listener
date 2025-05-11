use anyhow::Error as AnyError;
use tokio::sync::mpsc::{Receiver, Sender};
use listener::capture::{OwnedPacket, PCAPMeta, PacketCapturer};
use probe::iperf_json::IperfResponse;
use prost_net::bandwidth_server::PbfMsg;
use surge_ping::SurgeError;
use std::error::Error;

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

pub type CapEventSender = Sender<CapEvent>;
pub type CapEventReceiver = Receiver<CapEvent>;
pub type CaptureResult = Result<(PacketCapturer, PCAPMeta), Box<dyn Error>>;

pub mod proto_bw {
    tonic::include_proto!("bandwidth");
}

pub mod core_proto {
    tonic::include_proto!("core");
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
    pub const PRECISION: pcap::Precision = pcap::Precision::Micro;
    pub const TCP_STREAM_TIMEOUT: Duration = Duration::from_secs(20); //from_secs(900);
    pub const CLEANUP_INTERVAL: Duration = Duration::from_secs(10);
    pub const BURST_SIZE: usize = 100; // Limit buffered packets to 100 in individual trackers
    pub const SNAPLEN: i32 = 60 + 14 + 60; // Max header size=134 bytes.
    const IPV6HDR: i32 = 40;
}

pub enum CapEvent {
    Packet(OwnedPacket),
    IperfResponse(IperfResponse),
    Protobuf(PbfMsg),
    PathloadResponse(String),
    PingResponse(Result<Duration, SurgeError>),
    Error(AnyError),
}
