use futures::StreamExt;
use prost::Message;
use std::env;
use std::error::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

// Adjust the module path to match your generated protobuf code.
use network_listener::proto_bw::BandwidthMessage;

/// HTTP handler for uploading bandwidth metrics.
///
/// This endpoint expects a JSON payload that corresponds to your BandwidthMessage.
/// For each contained LinkState, we insert a row into the PostgreSQL database.
async fn upload_bandwidth(msg: BandwidthMessage) {
    // Connect to the database.
    let (client, _) = tokio_postgres::connect(
        "host=localhost, user=user, password=password, dbname=metricsdb",
        tokio_postgres::NoTls,
    )
    .await
    .unwrap();

    // For each LinkState record in the message, insert a row.
    for ls in &msg.link_state {
        // Convert the timestamp (assumed seconds since epoch) to a DateTime<Utc>
        // Using timestamp_opt for safety:
        let timestamp = ls.timestamp;

        // Now use dt directly in your query.
        if let Err(e) = client
            .execute(
                "INSERT INTO link_state (
                    sender_ip, receiver_ip, thp_in, thp_out, bw, abw, latency, delay, jitter, loss, ts
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                &[
                    &ls.sender_ip,
                    &ls.receiver_ip,
                    &ls.thp_in,
                    &ls.thp_out,
                    &ls.bw,
                    &ls.abw,
                    &ls.latency,
                    &ls.delay,
                    &ls.jitter,
                    &ls.loss,
                    &timestamp,
                ],
            )
            .await
        {
            eprintln!("Error inserting record: {}", e);
        }
    }
}

async fn handle_connection(socket: TcpStream) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Wrap the socket with a length-delimited codec for framing.
    let mut framed = Framed::new(socket, LengthDelimitedCodec::new());

    // Wait for a complete frame (a complete Protobuf message)
    if let Some(frame) = framed.next().await {
        let bytes = frame?;
        let msg = BandwidthMessage::decode(bytes)?;
        upload_bandwidth(msg).await;
    }
    Ok(())
}

async fn run_server(listen_addr: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let listener = TcpListener::bind(listen_addr).await?;
    println!("Server listening on {}", listen_addr);

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("Accepted connection from {}", addr);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket).await {
                eprintln!("Error handling connection from {}: {}", addr, e);
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Usage:
    //   cargo run -- <listen_addr>
    //
    // Example:
    //   cargo run -- 127.0.0.1:8080
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <listen_addr>", args[0]);
        return Ok(());
    }

    println!("{:?}", args);

    let listen_addr = args[1].clone();
    run_server(&listen_addr).await?;
    Ok(())
}
