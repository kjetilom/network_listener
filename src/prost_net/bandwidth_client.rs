use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use crate::proto_bw;
use proto_bw::bandwidth_service_client::BandwidthServiceClient;
use proto_bw::{HelloRequest, HelloReply};

#[derive(Debug)]
pub enum ClientEvent {
    /// Sends a hello message to the given IP. The provided `reply_tx` will receive
    /// the result of the operation.
    SendHello {
        ip: String,
        message: String,
        reply_tx: mpsc::Sender<Result<HelloReply, tonic::Status>>,
    },
    Stop,
}

/// Spawns the client in a background task that waits for events.
/// Each SendHello event is handled in its own task with a timeout,
/// so a slow connection or request won’t block further events.
pub fn spawn_client_task() -> (mpsc::Sender<ClientEvent>, tokio::task::JoinHandle<()>) {
    // Create a channel to send client events.
    let (tx, mut rx) = mpsc::channel::<ClientEvent>(100);

    // Spawn the main event loop.
    let handle = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                ClientEvent::SendHello { ip, message, reply_tx } => {
                    // Spawn a separate task for this request.
                    tokio::spawn(async move {
                        // Define the maximum allowed time for the request.
                        let timeout_duration = Duration::from_secs(5);
                        let addr = format!("http://{}:50051", ip);

                        // Wrap the connection and request in a timeout.
                        let result = timeout(timeout_duration, async {
                            // Attempt to connect to the target server.
                            let mut client = BandwidthServiceClient::connect(addr)
                                .await
                                .map_err(|e| {
                                    tonic::Status::unknown(format!("Connection failed: {:?}", e))
                                })?;

                            // Build and send the hello request.
                            let request = tonic::Request::new(HelloRequest { name: message });
                            client.say_hello(request).await.map(|res| res.into_inner())
                        })
                        .await;

                        match result {
                            Ok(inner_result) => {
                                // inner_result is a Result<HelloReply, tonic::Status>
                                let _ = reply_tx.send(inner_result).await;
                            }
                            Err(_) => {
                                // Timeout expired
                                let _ = reply_tx
                                    .send(Err(tonic::Status::deadline_exceeded("Request timed out")))
                                    .await;
                            }
                        }
                    });
                }
                ClientEvent::Stop => {
                    break;
                }
            }
        }
    });

    (tx, handle)
}
// // Example usage: spawns client task, sends some events, then does other work.
// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let (tx, handle) = spawn_client_task();

//     // Send an event to trigger sending "Hello"
//     tx.send(ClientEvent::SendHello("Tonic".into())).await?;
//     tx.send(ClientEvent::SendHello("Tonic".into())).await?;
//     tx.send(ClientEvent::SendHello("Tonic".into())).await?;
//     println!("Sent hello requests, sleeping for a bit...");
//     sleep(Duration::from_secs(1)).await;
//     println!("Woke up, sending stop event.");
//     tx.send(ClientEvent::Stop).await?;

//     // The main thread can do other async operations here.
//     // When done, you can drop `tx` or send more events.

//     // Optionally, wait for the client task to finish (e.g. if you want a graceful shutdown).
//     handle.await?;
//     println!("Client task finished.");
//     Ok(())
// }