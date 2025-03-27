use crate::probe::iperf::dispatch_iperf_client;
use crate::probe::pathload::dispatch_pathload_client;
use crate::proto_bw::DataMsg;
use crate::{proto_bw, CapEvent, CapEventSender};
use anyhow::{Error, Result};
use futures::future::join_all;
use log::info;
use proto_bw::bandwidth_service_client::BandwidthServiceClient;
use proto_bw::{HelloReply, HelloRequest};
use std::collections::HashMap;
use std::net::IpAddr;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time::{timeout, Duration, Instant};

use bytes::BytesMut;
use futures::SinkExt;
use prost::Message;
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

/// Events that the client task can respond to.
#[derive(Debug)]
pub enum ClientEvent {
    /// Sends a hello message to the given IP.
    /// The provided `reply_tx` will receive the result.
    SendHello { message: String },
    /// Stops the client task.
    Stop,
}

pub enum ClientHandlerEvent {
    InitClients { ips: Vec<IpAddr> },
    SendHello { ip: IpAddr, message: String },
    BroadcastHello { message: String },
    Stop,
    DoIperf3(String, u16, u16),
    DoPathloadTest(String),
    SendDataMsg(DataMsg),
}

pub enum ClientStatus {
    Connected(Instant),
    Disconnected(Instant),
}

impl ClientStatus {
    pub fn new_connected() -> Self {
        ClientStatus::Connected(Instant::now())
    }
    pub fn new_disconnected() -> Self {
        ClientStatus::Disconnected(Instant::now())
    }

    pub fn duration_since_now(&self) -> Duration {
        let other = Instant::now();
        match self {
            ClientStatus::Connected(t) => t.duration_since(other),
            ClientStatus::Disconnected(t) => t.duration_since(other),
        }
    }
}

#[derive(Debug)]
pub enum ClientEventResult {
    HelloReply(Result<HelloReply, tonic::Status>),
    ServerConnectError(Error),
    ServerConnected(String),
}

pub type OuterClient = (Sender<ClientEvent>, tokio::task::JoinHandle<()>);

pub struct BwClient {
    event_rx: Receiver<ClientEvent>,
    reply_tx: Sender<ClientEventResult>,
    connection: BandwidthServiceClient<tonic::transport::Channel>,
    status: Option<ClientStatus>,
}

pub struct ClientHandler {
    clients: HashMap<IpAddr, Option<OuterClient>>,
    reply_tx: Sender<ClientEventResult>,
    event_rx: Receiver<ClientHandlerEvent>,
    cap_ev_tx: CapEventSender,
}

impl ClientHandler {
    pub fn new(
        reply_tx: Sender<ClientEventResult>,
        event_rx: Receiver<ClientHandlerEvent>,
        cap_ev_tx: CapEventSender,
    ) -> Self {
        ClientHandler {
            clients: HashMap::new(),
            reply_tx,
            event_rx,
            cap_ev_tx,
        }
    }

    pub fn dispatch_client_handler(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.start_event_loop().await;
        })
    }

    async fn send_hello(&mut self, ip: IpAddr, message: String) {
        // Send hello to all clients
        if let Some(outer) = self.clients.get_mut(&ip) {
            if let Some((tx, _)) = outer {
                tx.send(ClientEvent::SendHello { message }).await.unwrap();
            } else {
                info!("Tried to send hello to uninitiated client {}", ip);
            }
        } else {
            info!("Tried to send hello to non-existent client {}", ip);
        }
    }

    pub async fn start_event_loop(mut self) {
        while let Some(event) = self.event_rx.recv().await {
            match event {
                ClientHandlerEvent::SendHello { ip, message } => {
                    // Send hello to all clients
                    self.send_hello(ip, message).await;
                }
                ClientHandlerEvent::Stop => break,
                ClientHandlerEvent::InitClients { ips } => {
                    self.init_clients(ips).await;
                }
                ClientHandlerEvent::BroadcastHello { message } => {
                    let ips: Vec<IpAddr> = self.clients.keys().cloned().collect();
                    for ip in ips {
                        self.send_hello(ip, message.clone()).await;
                    }
                }
                ClientHandlerEvent::DoIperf3(ip, port, duration) => {
                    dispatch_iperf_client(ip, port, duration, self.cap_ev_tx.clone());
                }
                ClientHandlerEvent::DoPathloadTest(ip) => {
                    dispatch_pathload_client(self.cap_ev_tx.clone(), ip);
                }
                ClientHandlerEvent::SendDataMsg(bw) => {
                    let cap_ev_tx = self.cap_ev_tx.clone();
                    tokio::spawn(async move {
                        send_message(
                            &format!(
                                "{}:{}",
                                &crate::CONFIG.server.ip,
                                &crate::CONFIG.server.port
                            ),
                            bw,
                            cap_ev_tx,
                        )
                        .await;
                    });
                }
            }
        }
    }

    /// For each IP address, run BwClient::new concurrently.
    /// Then, wait for all tasks to finish and store the returned client handles.
    pub async fn init_clients(&mut self, ips: Vec<IpAddr>) {
        let mut tasks = Vec::new();

        for ip in ips {
            if self.clients.contains_key(&ip) {
                continue;
            }
            let reply_txc = self.reply_tx.clone();
            // Clone the IP so we can return it along with the client.
            let ip_clone = ip;
            let ip_str = ip.to_string();

            // Spawn a task that calls BwClient::new and returns (IpAddr, OuterClient).
            tasks.push(tokio::spawn(async move {
                let client_tuple = BwClient::new(ip_str, reply_txc).await;
                (ip_clone, client_tuple)
            }));
        }

        // Wait for all tasks to complete.
        let results = join_all(tasks).await.into_iter();

        for res in results {
            match res {
                Ok((ip, client_result)) => match client_result {
                    Ok((client_handle, client_tx)) => {
                        self.clients.insert(ip, Some((client_tx, client_handle)));
                    }
                    Err(e) => {
                        self.reply_tx
                            .send(ClientEventResult::ServerConnectError(e))
                            .await
                            .unwrap();
                    }
                },
                Err(e) => {
                    self.reply_tx
                        .send(ClientEventResult::ServerConnectError(e.into()))
                        .await
                        .unwrap();
                }
            }
        }
    }
}

impl BwClient {
    pub async fn send_hello(&mut self, message: String) {
        // On self.connection, send a hello request
        let request = tonic::Request::new(HelloRequest { name: message });

        let response =
            match timeout(Duration::from_secs(3), self.connection.say_hello(request)).await {
                Ok(Ok(response)) => response.into_inner(),
                Ok(Err(e)) => {
                    self.status = Some(ClientStatus::new_disconnected());
                    self.reply_tx
                        .send(ClientEventResult::HelloReply(Err(e)))
                        .await
                        .unwrap();
                    return;
                }
                Err(_) => {
                    self.status = Some(ClientStatus::new_disconnected());
                    return;
                }
            };
        // let response = self.connection.say_hello(request);

        self.reply_tx
            .send(ClientEventResult::HelloReply(Ok(response)))
            .await
            .unwrap();
        self.status = Some(ClientStatus::new_connected());
    }

    pub async fn send_hello_noreply(&mut self, message: String) -> Result<HelloReply, Error> {
        // On self.connection, send a hello request, dont send reply to reply_tx
        let request = tonic::Request::new(HelloRequest { name: message });

        let response =
            match timeout(Duration::from_secs(3), self.connection.say_hello(request)).await {
                Ok(Ok(response)) => response.into_inner(),
                Ok(Err(e)) => {
                    self.status = Some(ClientStatus::new_disconnected());
                    return Err(e.into());
                }
                Err(_) => {
                    self.status = Some(ClientStatus::new_disconnected());
                    return Err(anyhow::anyhow!("Request timed out"));
                }
            };
        self.status = Some(ClientStatus::new_connected());
        Ok(response)
    }

    pub async fn start_event_loop(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(event) = self.event_rx.recv().await {
                match event {
                    ClientEvent::SendHello { message } => {
                        self.send_hello(message).await;
                    }
                    ClientEvent::Stop => break,
                }
            }
        })
    }

    pub async fn new(
        ip: String,
        reply_tx: Sender<ClientEventResult>,
    ) -> Result<(tokio::task::JoinHandle<()>, Sender<ClientEvent>)> {
        let (tx, rx) = channel::<ClientEvent>(10);
        let addr = format!("http://{}:{}", ip, crate::CONFIG.client.listen_port);
        let connect_timeout = Duration::from_secs(3);
        let connection = match timeout(connect_timeout, BandwidthServiceClient::connect(addr)).await
        {
            Ok(Ok(conn)) => conn,
            Ok(Err(e)) => {
                return Err(e.into());
            }
            Err(_) => {
                return Err(anyhow::anyhow!("Connection timed out, ip:{}", ip));
            }
        };

        let client = BwClient {
            event_rx: rx,
            reply_tx,
            connection,
            status: None,
        };

        client
            .reply_tx
            .send(ClientEventResult::ServerConnected(ip))
            .await
            .unwrap();

        let handle = client.start_event_loop().await;

        Ok((handle, tx))
    }
}

/// Sends a HelloMessage to the given peer address.
pub async fn send_message(peer_addr: &str, message: DataMsg, cap_ev_tx: CapEventSender) {
    let res = async move {
        let stream = match timeout(Duration::from_secs(4), TcpStream::connect(peer_addr)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                return Err(e.into());
            }
            Err(_) => {
                return Err(anyhow::anyhow!("Connection timed out"));
            }
        };
        let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

        // Create and encode a HelloMessage.
        let mut buf = BytesMut::with_capacity(message.encoded_len());
        message.encode(&mut buf)?;

        // Send the length-delimited message.
        framed.send(buf.freeze()).await?;
        Ok(())
    }
    .await;

    if let Err(e) = res {
        // Ignore send errors, as the receiver may have disconnected.
        cap_ev_tx.send(CapEvent::Error(e.into())).unwrap_or(());
    }
}
