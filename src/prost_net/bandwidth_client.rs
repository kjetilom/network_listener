use proto_bw::bandwidth_service_client::BandwidthServiceClient;
use proto_bw::HelloRequest;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

pub mod proto_bw {
    tonic::include_proto!("bandwidth"); // Matches the package name in .proto
}

// Events the client task can respond to.
#[derive(Debug)]
pub enum ClientEvent {
    SendHello(String),
    Stop,
    // You could add more event types here as needed.
}

// Spawns the client in a background task that waits for events.
pub fn spawn_client_task() -> (mpsc::Sender<ClientEvent>, tokio::task::JoinHandle<()>) {
    // Create a channel to send client events.
    let (tx, mut rx) = mpsc::channel::<ClientEvent>(100);

    // Spawn the task in the background.
    let handle = tokio::spawn(async move {
        // Create a single persistent gRPC client for efficiency.
        let mut client = match BandwidthServiceClient::connect("http://[::1]:50051").await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to connect to server: {:?}", e);
                return;
            }
        };

        // Enter an event loop, waiting for new events.
        while let Some(event) = rx.recv().await {
            match event {
                ClientEvent::SendHello(destination) => {
                    let request = tonic::Request::new(HelloRequest {
                        name: destination,
                    });
                    match client.say_hello(request).await {
                        Ok(response) => {
                            println!("Received greeting: {:?}", response.into_inner());
                        }
                        Err(e) => {
                            eprintln!("Error while sending hello: {:?}", e);
                        }
                    }
                }
                ClientEvent::Stop => {
                    // Stop the client task.
                    break;
                }
            }
        }
    });

    (tx, handle)
}

// Example usage: spawns client task, sends some events, then does other work.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (tx, handle) = spawn_client_task();

    // Send an event to trigger sending "Hello"
    tx.send(ClientEvent::SendHello("Tonic".into())).await?;
    tx.send(ClientEvent::SendHello("Tonic".into())).await?;
    tx.send(ClientEvent::SendHello("Tonic".into())).await?;
    println!("Sent hello requests, sleeping for a bit...");
    sleep(Duration::from_secs(1)).await;
    println!("Woke up, sending stop event.");
    tx.send(ClientEvent::Stop).await?;

    // The main thread can do other async operations here.
    // When done, you can drop `tx` or send more events.

    // Optionally, wait for the client task to finish (e.g. if you want a graceful shutdown).
    handle.await?;
    println!("Client task finished.");
    Ok(())
}