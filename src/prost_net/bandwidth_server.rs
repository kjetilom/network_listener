use anyhow::Result;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tonic::{transport::Server, Request, Response, Status};

use proto_bw::bandwidth_service_server::{BandwidthService, BandwidthServiceServer};
use proto_bw::{BandwidthMessage, BandwidthRequest, HelloReply, HelloRequest};

use crate::listener::capture::PCAPMeta;
use crate::proto_bw;
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
    sender: UnboundedSender<CapEvent>,
    pcap_meta: Arc<PCAPMeta>,
}

impl BwServer {
    pub fn new(sender: UnboundedSender<CapEvent>, pcap_meta: Arc<PCAPMeta>) -> Self {
        BwServer { sender, pcap_meta }
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
    type SubscribeBandwidthStream =
        Pin<Box<dyn Stream<Item = Result<BandwidthMessage, Status>> + Send + 'static>>;

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
            .expect("Failed to send protobuf message");

        Ok(Response::new(reply))
    }

    async fn get_bandwidth(
        &self,
        _: Request<BandwidthRequest>,
    ) -> Result<Response<BandwidthMessage>, Status> {
        panic!("Not implemented yet");
    }

    async fn subscribe_bandwidth(
        &self,
        _: Request<BandwidthRequest>,
    ) -> Result<Response<Self::SubscribeBandwidthStream>, Status> {
        panic!("Not implemented yet");
    }
}
