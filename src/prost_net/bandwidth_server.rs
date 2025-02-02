
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tonic::{transport::Server, Request, Response, Status};
use anyhow::Result;
use std::net::IpAddr;
use std::pin::Pin;
use futures::Stream;

use proto_bw::{BandwidthMessage, BandwidthRequest, HelloReply, HelloRequest};
use proto_bw::bandwidth_service_server::{BandwidthService, BandwidthServiceServer};

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
    my_ip: IpAddr,
}

impl BwServer {
    pub fn new(sender: UnboundedSender<CapEvent>, my_ip: IpAddr) -> Self {
        BwServer { sender, my_ip }
    }

    /// Spawns the server in the background.
    /// Consumes self, returns a handle to the task
    pub fn dispatch_server(self) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            let addr = "0.0.0.0:50051"
                .parse()
                .expect("Failed to parse address");

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
    type SubscribeBandwidthStream = Pin<Box<dyn Stream<Item = Result<BandwidthMessage, Status>> + Send + 'static>>;

    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        let inner = request.into_inner();
        let reply = HelloReply {
            ip_addr: self.my_ip.to_string(),
        };

        self.sender.send(CapEvent::Protobuf(PbfMsg::HelloRequest(inner))).expect("Failed to send protobuf message");

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
