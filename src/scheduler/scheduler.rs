use tokio::sync::mpsc::UnboundedReceiver;
use futures::StreamExt;
use network_listener::proto_bw::data_msg;
use network_listener::scheduler::core_grpc::{self, ThroughputDP};
use prost::Message;
use tokio_postgres::Client;
use std::error::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use clap::Parser;
use serde::Deserialize;

// Adjust the module path to match your generated protobuf code.
use network_listener::proto_bw::DataMsg;

use network_listener::scheduler::db_util::{upload_bandwidth, upload_probe_gap_measurements, upload_rtt, upload_throughput};

#[derive(Parser, Debug)]
#[command(name = "scheduler")]
struct Config {
    /// IP address and port to listen on, e.g. 127.0.0.1:8080
    #[arg(short, long)]
    listen_addr: String,

    /// Path to the secrets TOML file
    #[arg(short, long)]
    secrets_file: String,
}

#[derive(Deserialize)]
struct DbConfig {
    host: String,
    user: String,
    password: String,
    dbname: String,
}

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

async fn run_server(listen_addr: &str, client: Client, mut thput_rx: UnboundedReceiver<Vec<ThroughputDP>>) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Try three times to bind the address.
    let listener = {
        let mut attempts = 0;
        loop {
            match TcpListener::bind(listen_addr).await {
                Ok(listener) => break listener,
                Err(e) => {
                    if attempts < 3 {
                        println!("Failed to bind to {}: {}. Retrying...", listen_addr, e);
                        attempts += 1;
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    } else {
                        return Err(e.into());
                    }
                }
            }
        }
    };
    println!("Server listening on {}", listen_addr);
    loop {
        tokio::select! {
            Some(thput) = thput_rx.recv() => {
                // Process the throughput data
                println!("Received throughput data: {:?}", thput);
                upload_throughput(thput, &client).await;
            }
            Ok((socket, addr)) = listener.accept() => {
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
                        data_msg::Data::Pgmmsg(pgm) => {
                            upload_probe_gap_measurements(pgm, &client).await;
                        }
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Load the configuration from the command line arguments
    let config = Config::parse();

    let toml_content = std::fs::read_to_string(&config.secrets_file)?;
    let db_config: DbConfig = toml::from_str(&toml_content)?;

    // Set up the connection to the database
    let (client, connection) = tokio_postgres::connect(
        &format!(
            "host={} user={} password={} dbname={}",
            db_config.host, db_config.user, db_config.password, db_config.dbname
        ),
        tokio_postgres::NoTls,
    )
    .await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    let (thput_tx, thput_rx) = tokio::sync::mpsc::unbounded_channel();

    // start core_grpc listener
    let core_client = tokio::spawn(async move {
        core_grpc::start_listener(thput_tx).await.unwrap_or(());
    });

    let server = tokio::spawn(async move {
        run_server(&config.listen_addr, client, thput_rx).await.unwrap_or(());
    });

    // Wait for both tasks to finish
    let _ = tokio::try_join!(core_client, server);
    Ok(())
}
