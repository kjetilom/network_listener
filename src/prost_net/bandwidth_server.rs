use anyhow::Result;
use tokio::sync::mpsc::channel;
use tokio_stream::StreamExt;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tonic::{transport::Server, Request, Response, Status};

use proto_bw::bandwidth_service_server::{BandwidthService, BandwidthServiceServer};
use proto_bw::{BandwidthMessage, BandwidthRequest, HelloReply, HelloRequest};
use tokio_stream::wrappers::{ReceiverStream, BroadcastStream};
use tokio::sync::broadcast::Sender;

use crate::listener::capture::PCAPMeta;
use crate::proto_bw::DataMsg;
use crate::{proto_bw, CapEventSender};
use crate::CapEvent;

#[derive(Debug)]
pub enum PbfMsg {
    HelloReply(HelloReply),
    HelloRequest(HelloRequest),
    BandwidthMessage(BandwidthMessage),
    BandwidthRequest(BandwidthRequest),
}

#[derive(Debug)]
pub struct BwServer {
    sender: CapEventSender,
    pcap_meta: Arc<PCAPMeta>,
    bw_tx_stream: Arc<Sender<DataMsg>>,
}

impl BwServer {
    pub fn new(sender: CapEventSender, pcap_meta: Arc<PCAPMeta>, bw_tx_stream:  Arc<Sender<DataMsg>>) -> Self {
        BwServer { sender, pcap_meta, bw_tx_stream }
    }

    /// Spawns the server in the background.
    /// Consumes self, returns a handle to the task
    pub fn dispatch_server(self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            let addr = format!("0.0.0.0:{}", crate::CONFIG.client.listen_port).parse().expect("Failed to parse address");

            Server::builder()
                .add_service(BandwidthServiceServer::new(self))
                .serve(addr)
                .await?;
            Ok(())
        })
    }
}

#[tonic::async_trait]
impl BandwidthService for BwServer {
    type SubscribeBandwidthStream = ReceiverStream<Result<DataMsg, Status>>;

    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        let inner = request.into_inner();
        let reply = HelloReply {
            ip_addr: self.pcap_meta.ipv4.to_string(),
        };

        self.sender
            .send(CapEvent::Protobuf(PbfMsg::HelloRequest(inner)))
            .await.expect("Failed to send protobuf message");

        Ok(Response::new(reply))
    }

    async fn get_bandwidth(
        &self,
        _: Request<BandwidthRequest>,
    ) -> Result<Response<DataMsg>, Status> {
        panic!("Not implemented yet");
    }

    /// Handler for the SubscribeBandwidth RPC.
    /// This will subscribe to the broadcast channel for DataMsg and stream these
    /// to the client asking for data.
    async fn subscribe_bandwidth(
        &self,
        _: Request<BandwidthRequest>,
    ) -> Result<Response<Self::SubscribeBandwidthStream>, Status> {
        let (tx, rx) = channel::<Result<DataMsg, Status>>(16);

        let mut bc_stream = BroadcastStream::from(self.bw_tx_stream.subscribe());

        tokio::spawn(async move {
            while let Some(item) = bc_stream.next().await {
                let out = match item {
                    Ok(msg) => Ok(msg),
                    Err(e) => {
                        Err(Status::internal(format!("Error: {}", e)))
                    }
                };
                if tx.send(out).await.is_err() {
                    // receiver dropped
                    break;
                }
            }
        });
        let stream = ReceiverStream::new(rx);
        Ok(Response::new(stream))
    }
}
