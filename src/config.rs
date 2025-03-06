use std::path::Path;
use serde::Deserialize;
use std::fs;
use clap::Parser;

pub const DEFAULT_CONFIG_PATH: &str = "../mgensh/config/config.toml";


#[derive(Deserialize)]
pub struct AppConfig {
    pub client: Client,
    pub server: Server,
}


#[derive(Deserialize)]
pub struct Client {
    pub ip: Option<String>,
    pub iface: Option<String>,
    pub listen_port: Option<u16>,
}


#[derive(Deserialize)]
pub struct Server {
    #[serde(default = "default_server")]
    pub ip: String,
    #[serde(default = "default_server_port")]
    pub port: u16,
}

fn default_server() -> String {
    String::from("172.16.0.254")
}

fn default_server_port() -> u16 {
    40042
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
            listen_port: None,
        }
    }
}

impl Default for Server {
    fn default() -> Self {
        Server {
            ip: default_server(),
            port: default_server_port(),
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