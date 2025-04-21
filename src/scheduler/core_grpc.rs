use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use crate::core_proto::Session as CoreSession;
use crate::core_proto::core_api_client::CoreApiClient;
use crate::core_proto::{
    GetSessionRequest, LinkOptions, ThroughputsEvent,
    ThroughputsRequest,
};

use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Mutex;
use tonic::Streaming;

#[derive(Debug)]
struct Node {
    _id: i32,
    name: String,
    interfaces: HashMap<i32, Interface>,
}

#[derive(Debug)]
struct Session {
    _id: i32,
    nodes: HashMap<i32, Node>,
    links: HashMap<(i32, i32), Link>,
}

#[derive(Debug)]
struct Interface {
    id: i32,
    ip4: String,
    _ip6: String,
    _mac: String,
    name: String,
}

#[derive(Debug)]
struct Link {
    node1_id: i32,
    iface1: i32,
    node2_id: i32,
    iface2: i32,
    _options: Option<LinkOptions>,
}

fn build_session(session: CoreSession) -> Session {
    let mut core_session = Session {
        _id: session.id,
        nodes: session
            .nodes
            .iter()
            .map(|node| {
                (
                    node.id,
                    Node {
                        _id: node.id,
                        name: node.name.clone(),
                        interfaces: HashMap::new(),
                    },
                )
            })
            .collect(),
        links: HashMap::new(),
    };

    let links: HashMap<(i32, i32), Link> = session
        .links
        .iter()
        .map(|link| {
            let (iface1, iface1_id) = match link.iface1.clone() {
                Some(iface) => (
                    Some(Interface {
                        id: iface.id,
                        ip4: iface.ip4.clone(),
                        _ip6: iface.ip6.clone(),
                        _mac: iface.mac.clone(),
                        name: iface.name.clone(),
                    }),
                    iface.id,
                ),
                None => (None, -1),
            };

            let (iface2, iface2_id) = match link.iface2.clone() {
                Some(iface) => (
                    Some(Interface {
                        id: iface.id,
                        ip4: iface.ip4.clone(),
                        _ip6: iface.ip6.clone(),
                        _mac: iface.mac.clone(),
                        name: iface.name.clone(),
                    }),
                    iface.id,
                ),
                None => (None, -1),
            };
            match core_session.nodes.get_mut(&link.node1_id) {
                Some(node) => {
                    node.interfaces
                        .insert(iface1.as_ref().map(|i| i.id).unwrap_or(-1), iface1.unwrap());
                }
                None => {
                    eprintln!("Node {} not found", link.node1_id);
                }
            };

            match core_session.nodes.get_mut(&link.node2_id) {
                Some(node) => {
                    node.interfaces
                        .insert(iface2.as_ref().map(|i| i.id).unwrap_or(-1), iface2.unwrap());
                }
                None => {
                    eprintln!("Node {} not found", link.node2_id);
                }
            };

            (
                (link.node1_id, link.node2_id),
                Link {
                    node1_id: link.node1_id,
                    iface1: iface1_id,
                    node2_id: link.node2_id,
                    iface2: iface2_id,
                    _options: link.options,
                },
            )
        })
        .collect();

    core_session.links = links;
    core_session
}

// CORE listens on port 50051
pub async fn start_listener(tx: UnboundedSender<Vec<ThroughputDP>>) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = CoreApiClient::connect("http://127.0.0.1:50051").await?;

    let throughputs_request = ThroughputsRequest { session_id: 1 };

    let session_request = GetSessionRequest { session_id: 1 };

    let session = match client
        .get_session(session_request)
        .await?
        .into_inner()
        .session
    {
        Some(session) => session,
        None => {
            eprintln!("No session found");
            return Ok(());
        }
    };

    let core_session = build_session(session);

    // Wrap session in a mutex structure.
    let session = Arc::new(Mutex::new(core_session));
    let session_clone = session.clone();

    let thput_handle = tokio::spawn(async move {
        let response = client.throughputs(throughputs_request).await;
        match response {
            Ok(response) => {
                thput_event_loop(response.into_inner(), session_clone, tx).await;
            }
            Err(e) => {
                eprintln!("Error: {:?}", e);
            }
        }
    });

    // let tcp_sender_handle = tokio::spawn(async move {
    //     start_tcp_sender(session).await;
    // });

    // Wait for the throughput event loop to finish
    thput_handle.await.unwrap();
    // tcp_sender_handle.await.unwrap();

    Ok(())
}

#[derive(Debug)]
pub struct ThroughputDP {
    pub node1: String,
    pub iface1: String,
    pub ip41: String,
    pub node2: String,
    pub iface2: String,
    pub ip42: String,
    pub throughput: f64,
    pub timestamp: u128,
}

#[derive(Debug)]
pub struct ThroughputDps {
    pub dps: Vec<ThroughputDP>,
}

async fn thput_event_loop(
    mut thput_event: Streaming<ThroughputsEvent>,
    session: Arc<Mutex<Session>>,
    tx: UnboundedSender<Vec<ThroughputDP>>,
) {
    // println!("node1,iface1,ip41,node2,iface2,ip42,throughput,timestamp");
    while let Some(event) = thput_event.message().await.unwrap() {
        let locked_session = session.lock().await;
        let mut thput_dps = Vec::new();
        event.iface_throughputs.iter().for_each(|iface_thpt| {
            let mut node_id = iface_thpt.node_id;
            if node_id > 9 {
                node_id = node_id - 6;
            }

            for link in locked_session.links.values() {
                let link = if link.node1_id == node_id && link.iface1 == iface_thpt.iface_id {
                    link
                } else if link.node2_id == node_id && link.iface2 == iface_thpt.iface_id {
                    link
                } else {
                    continue;
                };
                let node1 = match locked_session.nodes.get(&link.node1_id) {
                    Some(node) => node,
                    None => {
                        eprintln!("Node {} not found", link.node1_id);
                        continue;
                    }
                };
                let node2 = match locked_session.nodes.get(&link.node2_id) {
                    Some(node) => node,
                    None => {
                        eprintln!("Node {} not found", link.node2_id);
                        continue;
                    }
                };

                let iface1 = match node1.interfaces.get(&link.iface1) {
                    Some(iface) => iface,
                    None => {
                        //eprintln!("Interface {} not found in node {}", link.iface1, link.node1_id);
                        continue;
                    }
                };

                let iface2 = match node2.interfaces.get(&link.iface2) {
                    Some(iface) => iface,
                    None => {
                        //eprintln!("Interface {} not found in node {}", link.iface2, link.node2_id);
                        continue;
                    }
                };

                let dp = ThroughputDP {
                    node1: node1.name.clone(),
                    iface1: iface1.name.clone(),
                    ip41: iface1.ip4.clone(),
                    node2: node2.name.clone(),
                    iface2: iface2.name.clone(),
                    ip42: iface2.ip4.clone(),
                    throughput: iface_thpt.throughput,
                    timestamp: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_millis(),
                };
                thput_dps.push(dp);
            }
        });
        if thput_dps.is_empty() {
            continue;
        }
        match tx.send(std::mem::take(&mut thput_dps)) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error sending data: {:?}", e);
            }
        }
    }
}
