/// The purpose of this module is for data collection only, and is not a core
/// part of the tool itself.

use clap::Parser;
use network_listener::proto_bw::data_msg;
use network_listener::scheduler::core_grpc::{self, ThroughputDP};
use serde::Deserialize;
use std::error::Error;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_postgres::Client;
use network_listener::scheduler::receiving_server::DataReceiver;

use network_listener::scheduler::db_util::{
    upload_bandwidth, upload_probe_gap_measurements, upload_rtt, upload_throughput, get_and_insert_experiment,
};

#[derive(Parser, Debug)]
#[command(name = "scheduler")]
struct Config {
    /// IP address and port to listen on, e.g. 127.0.0.1:8080
    #[arg(short, long)]
    listen_addr: String,

    /// Path to the secrets TOML file
    #[arg(short, long)]
    secrets_file: String,

    /// Name of the experiment
    #[arg(short, long)]
    experiment_name: String,

    /// Description of the experiment
    #[arg(short, long)]
    description: String,
}

#[derive(Deserialize)]
struct DbConfig {
    host: String,
    user: String,
    password: String,
    dbname: String,
}

async fn run_server(
    listen_addr: &str,
    client: Client,
    mut thput_rx: UnboundedReceiver<Vec<ThroughputDP>>,
    experiment_name: String,
    experiment_description: String,
) -> Result<(), Box<dyn Error + Send + Sync>> {

    let listen_port = listen_addr
        .split(':')
        .last()
        .ok_or("Invalid listen address")?
        .parse::<u16>()
        .map_err(|_| "Invalid port number")?;

    // Get experiment ID
    let experiment_id = get_and_insert_experiment(&client, &experiment_name, &experiment_description).await?;

    println!("Experiment ID: {}", experiment_id);
    let (data_tx, mut data_rx) = tokio::sync::mpsc::channel(40);
    let data_receiver = DataReceiver::new(data_tx);
    data_receiver.dispatch_server(listen_port.to_string());

    println!("Server listening on {}", listen_addr);

    loop {
        tokio::select! {
            Some(thput) = thput_rx.recv() => {
                // Process the throughput data
                upload_throughput(thput, &client, experiment_id).await;
            }

            // This just reads raw unencrypted TCP packets as protobuf data
            Some(bwm) = data_rx.recv() => {
                if let Some(data) = bwm.data {
                    match data {
                        data_msg::Data::Bandwidth(bw) => {
                            upload_bandwidth(bw, &client, experiment_id).await;
                        },
                        data_msg::Data::Hello(hello) => {
                            println!("Received hello message: {}", hello.message);
                        },
                        data_msg::Data::Rtts(rtts) => {
                            upload_rtt(rtts, &client, experiment_id).await;
                        }
                        data_msg::Data::Pgmmsg(pgm) => {
                            upload_probe_gap_measurements(pgm, &client, experiment_id).await;
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
        run_server(
            &config.listen_addr,
            client,
            thput_rx,
            config.experiment_name,
            config.description,
        )
        .await
        .unwrap_or(());
    });

    // Wait for both tasks to finish
    let _ = tokio::try_join!(core_client, server);
    Ok(())
}
