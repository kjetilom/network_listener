use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use futures::future::select_all;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{timeout, Duration};
use crate::proto_bw;
use proto_bw::bandwidth_service_client::BandwidthServiceClient;
use proto_bw::{HelloRequest, HelloReply};

/// Events that the client task can respond to.
#[derive(Debug)]
pub enum ClientEvent {
    /// Sends a hello message to the given IP.
    /// The provided `reply_tx` will receive the result.
    SendHello {
        message: String,
    },
    /// Stops the client task.
    Stop,
}

pub enum ClientHandlerEvent {
    InitClients {
        ips: Vec<IpAddr>,
    },
    SendHello {
        ip: IpAddr,
        message: String,
    },
    Stop,
}

pub type OuterClient = (mpsc::Sender<ClientEvent>, tokio::task::JoinHandle<()>);

pub struct BwClient {
    event_rx: mpsc::Receiver<ClientEvent>,
    reply_tx: mpsc::Sender<Result<HelloReply, tonic::Status>>,
    connection: BandwidthServiceClient<tonic::transport::Channel>,
}

pub struct ClientHandler {
    clients: HashMap<IpAddr, OuterClient>,
    reply_tx: mpsc::Sender<Result<HelloReply, tonic::Status>>,
    event_rx: mpsc::Receiver<ClientHandlerEvent>,
}

impl ClientHandler {
    pub fn new(reply_tx: mpsc::Sender<Result<HelloReply, tonic::Status>>, event_rx: mpsc::Receiver<ClientHandlerEvent>) -> Self {
        ClientHandler {
            clients: HashMap::new(),
            reply_tx,
            event_rx,
        }

    }

    pub fn dispatch_client_handler(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.start_event_loop().await;
        })
    }

    pub async fn start_event_loop(mut self) {
        while let Some(event) = self.event_rx.recv().await {
            match event {
                ClientHandlerEvent::SendHello { ip, message } => {
                    // Send hello to all clients
                    for (ip, oc) in self.clients.iter_mut() {
                        let (tx, _) = oc;
                        tx.send(ClientEvent::SendHello { message: message.clone() }).await.unwrap();
                    }
                }
                ClientHandlerEvent::Stop => break,
                ClientHandlerEvent::InitClients { ips } => {
                    self.init_clients(ips).await;
                }
            }
        }
    }

    pub fn add_client(&mut self, ip: IpAddr, oc: OuterClient) {
        self.clients.insert(ip, oc);
    }

    pub async fn init_clients(
        &mut self,
        ips: Vec<IpAddr>,
    ) -> JoinHandle<()> {
        // Collect join handles to later monitor.
        let mut join_handles = Vec::new();
        for ip in ips {
            let reply_txc = self.reply_tx.clone();
            let ipc = ip.to_string();
            join_handles.push(tokio::spawn(async {BwClient::new(ipc, reply_txc).await}));

            // Store the client in the handler's map.
        }

        // Spawn a task that waits for any of the client join handles to finish.
        tokio::spawn(async move {
            // `select_all` waits until one of the futures completes.
            let (finished_result, index, _remaining) = select_all(join_handles).await;
            // Add all the clients to self.clients
            let (handle, tx) = match finished_result {
                Ok((handle, tx)) => (handle, tx),
                Err(e) => {
                    eprintln!("Failed to initialize client: {:?}", e);
                    return;
                }
            };
            // You can add further logic here if you need to restart clients or cleanup.
        })
    }
}

impl BwClient {
    pub async fn send_hello(&mut self, message: String) {
        // On self.connection, send a hello request
        let request = tonic::Request::new(HelloRequest { name: message });

        let response = match timeout(Duration::from_secs(3), self.connection.say_hello(request)).await {
            Ok(Ok(response)) => response.into_inner(),
            Ok(Err(e)) => {
                eprintln!("Failed to send hello: {:?}", e); // This should be handled
                return;
            }
            Err(_) => {
                eprintln!("Request timed out"); // This should be handled
                return;
            }
        };
        // let response = self.connection.say_hello(request);

        self.reply_tx.send(Ok(response)).await.unwrap();
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

    pub async fn new(ip: String, reply_tx:mpsc::Sender<Result<HelloReply, tonic::Status>> ) -> (tokio::task::JoinHandle<()>, mpsc::Sender<ClientEvent>) {
        let (tx, rx) = mpsc::channel::<ClientEvent>(10);
        let addr = format!("http://{}:50051", ip);
        let connect_timeout = Duration::from_secs(3);
        let connection = match timeout(connect_timeout, BandwidthServiceClient::connect(addr)).await {
            Ok(Ok(conn)) => conn,
            Ok(Err(e)) => {
                eprintln!("Failed to connect to {}: {:?}", ip, e);
                return (tokio::spawn(async {}), tx);
            }
            Err(_) => {
                eprintln!("Connection to {} timed out", ip);
                return (tokio::spawn(async {}), tx);
            }
        };

        let client = BwClient {
            event_rx: rx,
            reply_tx,
            connection,
        };

        let handle = client.start_event_loop().await;

        (handle, tx)

    }
}


// /// Handles a single SendHello event.
// /// This function retrieves or creates a gRPC client for the given IP,
// /// sends a hello request with a timeout, and returns the result via the reply channel.
// async fn handle_send_hello(
//     ip: String,
//     message: String,
//     reply_tx: mpsc::Sender<Result<HelloReply, tonic::Status>>,
//     cache: Arc<Mutex<HashMap<String, BandwidthServiceClient<tonic::transport::Channel>>>>,
// ) {
//     const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
//     let addr = format!("http://{}:50051", ip);

//     // Try to retrieve an existing client without holding the lock for too long.
//     let maybe_client = {
//         let cache_lock = cache.lock().await;
//         cache_lock.get(&ip).cloned()
//     };

//     let mut client = if let Some(client) = maybe_client {
//         // Use the cached client.
//         client
//     } else {
//         // Attempt to create a new client, with a timeout.
//         match timeout(REQUEST_TIMEOUT, BandwidthServiceClient::connect(addr.clone())).await {
//             // Connection established within timeout.
//             Ok(Ok(new_client)) => {
//                 let mut cache_lock = cache.lock().await;
//                 cache_lock.insert(ip.clone(), new_client.clone());
//                 new_client
//             }
//             // Connection attempt failed.
//             Ok(Err(e)) => {
//                 eprintln!("Failed to connect to {}: {:?}", ip, e);
//                 let _ = reply_tx.send(Err(tonic::Status::unknown("Connection failed"))).await;
//                 return;
//             }
//             // Connection timed out.
//             Err(_) => {
//                 eprintln!("Connection to {} timed out", ip);
//                 let _ = reply_tx
//                     .send(Err(tonic::Status::deadline_exceeded("Connection timed out")))
//                     .await;
//                 return;
//             }
//         }
//     };

//     // Build and send the hello request.
//     let request = tonic::Request::new(HelloRequest { name: message });
//     let response_result = timeout(REQUEST_TIMEOUT, client.say_hello(request)).await;

//     match response_result {
//         Ok(inner_result) => {
//             let result = inner_result.map(|res| res.into_inner());
//             let _ = reply_tx.send(result).await;
//         }
//         Err(_) => {
//             let _ = reply_tx
//                 .send(Err(tonic::Status::deadline_exceeded("Request timed out")))
//                 .await;
//             // Remove the cached client on timeout to force a reconnection next time.
//             let mut cache_lock = cache.lock().await;
//             cache_lock.remove(&ip);
//         }
//     }
// }

// /// Spawns a client task that listens for events and handles SendHello requests.
// /// This version reuses connections by caching them in a shared HashMap.
// pub fn spawn_client_task() -> (mpsc::Sender<ClientEvent>, tokio::task::JoinHandle<()>) {
//     // Create an mpsc channel for sending client events.
//     let (tx, mut rx) = mpsc::channel::<ClientEvent>(100);

//     // Shared connection cache: maps IP addresses to connected clients.
//     let connection_cache: Arc<Mutex<HashMap<String, BandwidthServiceClient<tonic::transport::Channel>>>> =
//         Arc::new(Mutex::new(HashMap::new()));

//     // Spawn the main event loop.
//     let handle = tokio::spawn({
//         let connection_cache = Arc::clone(&connection_cache);
//         async move {
//             while let Some(event) = rx.recv().await {
//                 match event {
//                     ClientEvent::SendHello { ip, message, reply_tx } => {
//                         // For each SendHello event, spawn a new task to handle it.
//                         let cache = Arc::clone(&connection_cache);
//                         tokio::spawn(handle_send_hello(ip, message, reply_tx, cache));
//                     }
//                     ClientEvent::Stop => break,
//                 }
//             }
//         }
//     });

//     (tx, handle)
// }
