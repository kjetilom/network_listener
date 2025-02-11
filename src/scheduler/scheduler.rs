use chrono::{DateTime, Utc};
use futures::StreamExt;
use prost::Message;
use tokio_postgres::{Client, types::Timestamp};
use std::env;
use std::error::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

// Adjust the module path to match your generated protobuf code.
use network_listener::proto_bw::BandwidthMessage;

type TstampTZ = Timestamp<DateTime<Utc>>;

/// HTTP handler for uploading bandwidth metrics.
///
/// This endpoint expects a JSON payload that corresponds to your BandwidthMessage.
/// For each contained LinkState, we insert a row into the PostgreSQL database.
async fn upload_bandwidth(msg: BandwidthMessage, client: &Client) {
    // For each LinkState record in the message, insert a row.
    for ls in &msg.link_state {
        // Convert the timestamp (assumed seconds since epoch) to a DateTime<Utc>
        // Using timestamp_opt for safety:
        let timestamp = ls.timestamp;
        let ts = TstampTZ::Value(DateTime::from_timestamp_millis(timestamp).unwrap());

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
                    &ts,
                ],
            )
            .await
        {
            eprintln!("Error inserting record: {}", e);
        }
    }
}

async fn handle_connection(socket: TcpStream) -> Result<BandwidthMessage, Box<dyn Error + Send + Sync>> {
    // Wrap the socket with a length-delimited codec for framing.
    let mut framed = Framed::new(socket, LengthDelimitedCodec::new());

    // Wait for a complete frame (a complete Protobuf message)
    if let Some(frame) = framed.next().await {
        let bytes = match frame {
            Ok(bytes) => bytes,
            Err(e) => return Err(e.into()),
        };

        // Parse the message
        let msg = BandwidthMessage::decode(bytes);
        match msg {
            Ok(msg) => {
                return Ok(msg);
            }
            Err(e) => return Err(e.into()),
        }
    } else {
        return Err("No data received".into());
    }
}

async fn run_server(listen_addr: &str, client: Client) -> Result<(), Box<dyn Error + Send + Sync>> {
    let listener = TcpListener::bind(listen_addr).await?;
    println!("Server listening on {}", listen_addr);

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("Accepted connection from {}", addr);
        let bwm = tokio::spawn(async move {
            handle_connection(socket).await
        }).await??;
        upload_bandwidth(bwm, &client).await;
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

    println!("Connecting to database");
    let (client, connection) = tokio_postgres::connect(
        "host=localhost user=user password=password dbname=metricsdb",
        tokio_postgres::NoTls,
    ).await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    println!("{:?}", args);

    let listen_addr = args[1].clone();
    run_server(&listen_addr, client).await?;
    Ok(())
}
