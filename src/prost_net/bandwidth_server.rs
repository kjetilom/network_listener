use tonic::{transport::Server, Request, Response, Status};

use proto_bw::{HelloReply, HelloRequest};
use proto_bw::bandwidth_service_server::{BandwidthService, BandwidthServiceServer};

pub mod proto_bw {
    tonic::include_proto!("bandwidth"); // The string specified here must match the proto package name
}


#[derive(Debug, Default)]
pub struct MyGreeter {}

#[tonic::async_trait]
impl BandwidthService for MyGreeter {
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        println!("Got a request: {:?}", request);

        let reply = HelloReply {
            greeting: format!("Hello {}!", request.into_inner().name),
        };

        Ok(Response::new(reply))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let greeter = MyGreeter::default();

    Server::builder()
        .add_service(BandwidthServiceServer::new(greeter))
        .serve(addr)
        .await?;

    Ok(())
}