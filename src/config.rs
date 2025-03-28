use clap::Parser;
use serde::Deserialize;
use std::fs;
use std::{path::Path, time::Duration, u32};

#[derive(Deserialize, Debug)]
pub struct AppConfig {
    pub client: Client,
    pub server: Server,
}

#[derive(Deserialize, Debug)]
pub struct Client {
    pub ip: Option<String>,
    pub iface: Option<String>,
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,
    #[serde(default = "default_link_phy_cap")]
    pub link_phy_cap: u32,
    #[serde(
        default = "default_measurement_window",
        deserialize_with = "duration_deserialize"
    )]
    pub measurement_window: Duration,
    #[serde(
        default = "default_tstamp_type",
        deserialize_with = "tstamp_type_deserialize"
    )]
    pub tstamp_type: pcap::TimestampType,
    #[serde(
        default = "default_timestamp_precision",
        deserialize_with = "precision_deserialize"
    )]
    pub timestamp_precision: pcap::Precision,
}

#[derive(Deserialize, Debug)]
pub struct Server {
    #[serde(default = "default_server")]
    pub ip: String,
    #[serde(default = "default_server_port")]
    pub port: u16,
    #[serde(default = "default_send_rtts")]
    pub send_rtts: bool,
    #[serde(default = "default_send_link_states")]
    pub send_link_states: bool,
    #[serde(default = "default_send_pgm_dps")]
    pub send_pgm_dps: bool,
    #[serde(default = "default_probe_technique")]
    pub probe_technique: String,
}

fn default_server() -> String {
    String::from("172.16.0.254")
}
fn default_server_port() -> u16 {
    50041
}
fn default_listen_port() -> u16 {
    40042
}
fn default_measurement_window() -> Duration {
    Duration::from_secs(20)
}
fn default_link_phy_cap() -> u32 {
    u32::MAX
}
fn default_tstamp_type() -> pcap::TimestampType {
    pcap::TimestampType::Adapter
}
fn default_timestamp_precision() -> pcap::Precision {
    pcap::Precision::Micro
}
fn default_send_rtts() -> bool {
    false
}
fn default_send_link_states() -> bool {
    true
}
fn default_send_pgm_dps() -> bool {
    false
}
fn default_probe_technique() -> String {
    String::from("iperf3")
}

fn duration_deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = u32::deserialize(deserializer)?;
    Ok(Duration::from_secs(s as u64))
}

fn precision_deserialize<'de, D>(deserializer: D) -> Result<pcap::Precision, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.to_lowercase().as_str() {
        "micro" => Ok(pcap::Precision::Micro),
        "nano" => Ok(pcap::Precision::Nano),
        _ => Err(serde::de::Error::custom("Invalid timestamp precision")),
    }
}

fn tstamp_type_deserialize<'de, D>(deserializer: D) -> Result<pcap::TimestampType, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "adapter" => Ok(pcap::TimestampType::Adapter),
        "host" => Ok(pcap::TimestampType::Host),
        "host_lowprec" => Ok(pcap::TimestampType::HostLowPrec),
        "adapter_unsynced" => Ok(pcap::TimestampType::AdapterUnsynced),
        "host_highprec" => Ok(pcap::TimestampType::HostHighPrec),
        _ => Err(serde::de::Error::custom("Invalid timestamp type")),
    }
}



impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            client: Client::default(),
            server: Server::default(),
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Client {
            ip: None,
            iface: None,
            listen_port: default_listen_port(),
            link_phy_cap: default_link_phy_cap(),
            measurement_window: default_measurement_window(),
            tstamp_type: default_tstamp_type(),
            timestamp_precision: default_timestamp_precision(),
        }
    }
}

impl Default for Server {
    fn default() -> Self {
        Server {
            ip: default_server(),
            port: default_server_port(),
            send_rtts: default_send_rtts(),
            send_link_states: default_send_link_states(),
            send_pgm_dps: default_send_pgm_dps(),
            probe_technique: default_probe_technique(),
        }
    }
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[arg(short, long, default_value = "config.toml")]
    pub config: String,

    #[arg(long)]
    pub host: Option<String>,

    #[arg(long)]
    pub iface: Option<String>,
}

pub fn load_config() -> AppConfig {
    let cli_args = CliArgs::parse();
    let mut config = AppConfig::default();

    if Path::new(&cli_args.config).exists() {
        let contents = fs::read_to_string(&cli_args.config).expect("Failed to read config file");
        let file_config = toml::from_str(&contents).expect("Failed to parse config file");
        config = file_config;
    }

    if let Some(host) = cli_args.host {
        config.client.ip = Some(host);
    }

    if let Some(iface) = cli_args.iface {
        config.client.iface = Some(iface);
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.client.ip, None);
        assert_eq!(config.client.iface, None);
    }
}
