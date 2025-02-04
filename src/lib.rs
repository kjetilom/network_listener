use listener::capture::{OwnedPacket, PCAPMeta, PacketCapturer};
use probe::iperf_json::IperfResponse;
use prost_net::bandwidth_server::PbfMsg;
use std::error::Error;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub mod listener;
pub mod logging;
pub mod probe;
pub mod prost_net;

pub use listener::packet::*;
pub use listener::tracking::*;
pub use listener::Settings;
pub use prost_net::bandwidth_client::ClientEvent;

pub const IPERF3_PORT: u16 = 5001;
pub const PROTOBUF_PORT: u16 = 5012; // Unused

pub type CapEventSender = UnboundedSender<CapEvent>;
pub type CapEventReceiver = UnboundedReceiver<CapEvent>;
pub type CaptureResult = Result<(PacketCapturer, PCAPMeta), Box<dyn Error>>;

pub mod proto_bw {
    tonic::include_proto!("bandwidth"); // Matches the package name in .proto
}

pub enum CapEvent {
    Packet(OwnedPacket),
    IperfResponse(IperfResponse),
    Protobuf(PbfMsg),
}
