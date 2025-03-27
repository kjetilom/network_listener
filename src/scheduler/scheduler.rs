use futures::StreamExt;
use network_listener::proto_bw::data_msg;
use prost::Message;
use tokio_postgres::Client;
use std::env;
use std::error::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

// Adjust the module path to match your generated protobuf code.
use network_listener::proto_bw::DataMsg;

use network_listener::scheduler::db_util::{upload_bandwidth, upload_probe_gap_measurements, upload_rtt};

async fn handle_connection(socket: TcpStream) -> Result<DataMsg, Box<dyn Error + Send + Sync>> {
    // Wrap the socket with a length-delimited codec for framing.
    let mut framed = Framed::new(socket, LengthDelimitedCodec::new());

    // Wait for a complete frame (a complete Protobuf message)
    if let Some(frame) = framed.next().await {
        let bytes = match frame {
            Ok(bytes) => bytes,
            Err(e) => return Err(e.into()),
        };

        // Parse the message
        let msg = DataMsg::decode(bytes);
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
        // Unefficient, but simple: Connections are not maintained
        let (socket, addr) = listener.accept().await?;
        println!("Accepted connection from {}", addr);
        let bwm = tokio::spawn(async move {
            handle_connection(socket).await
        }).await??;

        if let Some(data) = bwm.data {
            match data {
                data_msg::Data::Bandwidth(bw) => {
                    upload_bandwidth(bw, &client).await;
                },
                data_msg::Data::Hello(hello) => {
                    println!("Received hello message: {}", hello.message);
                },
                data_msg::Data::Rtts(rtts) => {
                    upload_rtt(rtts, &client).await;
                }
                data_msg::Data::Pgm(pgm) => {
                    upload_probe_gap_measurements(pgm, &client).await;
                }
            }
        }

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
        "host=localhost user=user password=password dbname=metricsdb", // Very secure
        tokio_postgres::NoTls,
    ).await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    let listen_addr = args[1].clone();
    run_server(&listen_addr, client).await?;
    Ok(())
}
