/// Partial implementation of an active probing module using ICMP echo requests.
/// Needs further development and testing.
use std::collections::HashMap;
use std::net::IpAddr;
use rand::random;
use surge_ping::{Client, Config, PingIdentifier, PingSequence, SurgeError, ICMP};
use tokio::sync::mpsc;

use crate::{CapEvent, CapEventSender};

/// Commands sent to the PingManager.
pub enum PingCommand {
    /// Register a host
    Register {
        host: IpAddr,
        config: Config,
    },
    /// Send a ping to the host.
    Ping {
        host: IpAddr,
        seq: PingSequence,
        payload: Vec<u8>,
    },
}

/// Manages pingers for different hosts.
pub struct PingManager {
    // Stores an active pinger for each host.
    pingers: HashMap<IpAddr, surge_ping::Pinger>,
    clientv4: Client,
    clientv6: Client,
    sender: CapEventSender,
}

impl PingManager {
    pub fn new(sender: CapEventSender) -> Self {
        Self {
            pingers: HashMap::new(),
            clientv4: PingManager::default_config(ICMP::V4),
            clientv6: PingManager::default_config(ICMP::V6),
            sender,
        }
    }

    fn default_config(kind: ICMP) -> Client {
        Client::new(&Config::builder().kind(kind).build()).unwrap()
    }

    fn get_client(&self, host: &IpAddr) -> &Client {
        match host {
            IpAddr::V4(_) => &self.clientv4,
            IpAddr::V6(_) => &self.clientv6,
        }
    }

    /// Creates and stores a pinger for the given host using the provided Config.
    async fn create_pinger(&mut self, host: IpAddr, config: Config) -> Result<(), SurgeError> {
        let client = Client::new(&config)?;
        let pinger = client.pinger(host, PingIdentifier(random())).await;
        self.pingers.insert(host, pinger);
        Ok(())
    }

    /// Gets an existing pinger for the host or creates one with a default Config.
    async fn get_or_create_pinger(&mut self, host: IpAddr) -> Result<&mut surge_ping::Pinger, SurgeError> {
        if !self.pingers.contains_key(&host) {
            // Use a default configuration based on the IP type.
            let client = self.get_client(&host);
            let pinger = client.pinger(host, PingIdentifier(random())).await;
            self.pingers.insert(host, pinger);
        }
        Ok(self.pingers.get_mut(&host).unwrap())
    }

    /// Event loop for handling incoming ping commands.
    pub async fn run(mut self, mut rx: mpsc::Receiver<PingCommand>) {
        while let Some(cmd) = rx.recv().await {
            match cmd {
                PingCommand::Register { host, config } => {
                    let res = self.create_pinger(host, config).await;
                    if let Err(e) = res {
                        let _ = self.sender.send(CapEvent::PingResponse(Err(e)));
                    }
                }
                PingCommand::Ping { host, seq, payload } => {
                    let result = match self.get_or_create_pinger(host).await {
                        Ok(pinger) => {
                            pinger.ping(seq, &payload)
                                .await
                                .map(|(_packet, duration)| duration)
                        }
                        Err(e) => Err(e),
                    };
                    let _ = self.sender.send(CapEvent::PingResponse(result));
                }
            }
        }
    }
}
