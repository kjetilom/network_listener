pub struct Settings {}

impl Settings {
    pub const PROMISC: bool = true;
    pub const IMMEDIATE_MODE: bool = true;
    pub const TIMEOUT: i32 = 0;
    pub const TSTAMP_TYPE: pcap::TimestampType = pcap::TimestampType::Adapter;
    pub const PRESICION: pcap::Precision = pcap::Precision::Nano;
}

pub mod logger;
pub mod parser;
pub mod utils;
pub mod analyzer;
pub mod capture;
pub mod config;
pub mod tracker;
pub mod stream_id;
pub mod stream_manager;
