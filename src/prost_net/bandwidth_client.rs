use proto_bw::bandwidth_service_client::BandwidthServiceClient;
use proto_bw::HelloRequest;

pub mod proto_bw {
    tonic::include_proto!("bandwidth"); // The string specified here must match the proto package name
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = BandwidthServiceClient::connect("http://[::1]:50051").await?;

    let request = tonic::Request::new(HelloRequest {
        name: "Tonic".into(),
    });

    let response = client.say_hello(request).await?;

    println!("RESPONSE={:?}", response);

    Ok(())
}