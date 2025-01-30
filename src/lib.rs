pub mod listener;
pub mod logging;
pub mod probe;

pub mod tutorial {
    include!(concat!(env!("OUT_DIR"), "/network_listener.bandwidth.rs"));
}

pub const IPERF3_PORT: u16 = 5001;
pub const PROTOBUF_PORT: u16 = 5012; // Unused