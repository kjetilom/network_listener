use std::time::Duration;

pub struct Settings {}

impl Settings {
    pub const PROMISC: bool = true;
    pub const IMMEDIATE_MODE: bool = true;
    pub const TIMEOUT: i32 = 0;
    pub const TSTAMP_TYPE: pcap::TimestampType = pcap::TimestampType::Adapter;
    pub const PRECISION: pcap::Precision = pcap::Precision::Nano;
    pub const TCP_STREAM_TIMEOUT: Duration = Duration::from_secs(5); //from_secs(900);
    pub const SYN_ACK_TIMEOUT: Duration = Duration::from_secs(10); // 75
    pub const FIN_WAIT_TIMEOUT: Duration = Duration::from_secs(675);
    pub const CLEANUP_INTERVAL: Duration = Duration::from_secs(8);
    pub const LONGER_INTERVAL: Duration = Duration::from_secs(20);
    pub const SCHEDULER_DEST: &str = "172.16.0.254:50041";
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

pub mod capture;
pub mod config;
pub mod packet;
pub mod parser;
pub mod procfs_reader;
pub mod tracking;
