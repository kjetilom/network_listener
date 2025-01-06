use std::time::Duration;

pub struct Settings {}

impl Settings {
    pub const PROMISC: bool = true;
    pub const IMMEDIATE_MODE: bool = true;
    pub const TIMEOUT: i32 = 0;
    pub const TSTAMP_TYPE: pcap::TimestampType = pcap::TimestampType::Adapter;
    pub const PRECISION: pcap::Precision = pcap::Precision::Nano;
    pub const TCP_STREAM_TIMEOUT: Duration = Duration::from_secs(900);
    pub const SYN_ACK_TIMEOUT: Duration = Duration::from_secs(10); // 75
    pub const FIN_WAIT_TIMEOUT: Duration = Duration::from_secs(675);
    pub const CLEANUP_INTERVAL: Duration = Duration::from_secs(10); // 900
}
pub mod packet;
pub mod parser;
pub mod analyzer;
pub mod capture;
pub mod config;
pub mod tracker;
pub mod stream_id;
pub mod stream_manager;
pub mod procfs_reader;