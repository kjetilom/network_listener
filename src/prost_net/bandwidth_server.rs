use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tonic::{transport::Server, Request, Response, Status};

use proto_bw::{HelloReply, HelloRequest};
use proto_bw::bandwidth_service_server::{BandwidthService, BandwidthServiceServer};

use crate::*;

pub mod proto_bw {
    tonic::include_proto!("bandwidth"); // The string specified here must match the proto package name
}

#[derive(Debug)]
pub enum PbfMsg {
    HelloReply(HelloReply),
    HelloRequest(HelloRequest),
}


#[derive(Debug)]
pub struct BwServer {
    sender: UnboundedSender<CapEvent>,
}

impl BwServer {
    pub fn new(sender: UnboundedSender<CapEvent>) -> Self {
        BwServer { sender }
    }

    /// Spawns the server in the background.
    /// Consumes self, returns a handle to the task
    pub fn spawn_bw_server(self) -> JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>> {
        tokio::spawn(async move {
            let addr = "0.0.0.0:50051".parse()?;

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
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        let inner = request.into_inner();
        let reply = HelloReply {
            greeting: format!("Hello {}!", inner.name),
        };

        self.sender.send(CapEvent::Protobuf(PbfMsg::HelloRequest(inner))).expect("Failed to send protobuf message");

        Ok(Response::new(reply))
    }
}


// pub fn spawn_bw_server(sender: CapEventSender) -> JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>> {
//     tokio::spawn(async move {
//         let addr = "[::1]:50051".parse()?;
//         let service = BwServer { sender };

//         Server::builder()
//             .add_service(BandwidthServiceServer::new(service))
//             .serve(addr)
//             .await?;

//         Ok(())
//     })
// }
