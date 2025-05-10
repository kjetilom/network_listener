use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};

use anyhow::Result;
use log::info;
use tokio::process::Command;

use crate::probe::iperf_json::IperfResponse;
use crate::*;

/// Represents an `iperf3` server process that listens for incoming tests
/// and forwards parsed JSON results as `CapEvent::IperfResponse`.
#[derive(Debug)]
pub struct IperfServer {
    /// TCP port to listen on
    listen_port: u16,
    /// Channel to send parsed `CapEvent`s
    sender: CapEventSender,
}

impl IperfServer {
    /// Create a new `IperfServer` bound to `listen_port`.
    ///
    /// # Arguments
    /// * `listen_port` - port on which to run `iperf3 -s`
    /// * `sender` - channel for delivering `CapEvent::IperfResponse`
    pub fn new(listen_port: u16, sender: CapEventSender) -> Result<Self> {
        Ok(IperfServer {
            listen_port,
            sender,
        })
    }

    /// Launch the server loop on a Tokio task.
    ///
    /// Returns a `JoinHandle` resolving to `Result<()>` when the server stops.
    pub fn dispatch_server(self) -> tokio::task::JoinHandle<Result<()>> {
        tokio::spawn(async move { self.start().await })
    }

    /// Runs the `iperf3` server (`-s --json`), reads stdout line by line,
    /// buffers JSON objects, parses into `IperfResponse`, and sends
    /// each parsed result as `CapEvent::IperfResponse`.
    pub async fn start(self) -> Result<()> {
        // Run iperf -s -p $port
        let port = self.listen_port;
        info!("Starting iperf server on port {}", port);

        // Spawn iperf3 server process
        let mut cmd = Command::new("iperf3");
        cmd.args(["-s", "--json", "-p", &port.to_string()]);
        cmd.stdout(Stdio::piped());

        let mut child = cmd.spawn().expect("Failed to start iperf server");
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let mut reader = BufReader::new(stdout).lines();

        // Separate task to log exit status
        tokio::spawn(async move {
            let status = child.wait().await.expect("Failed to wait on child");
            info!("iperf server exited with: {}", status);
        });

        // Parse incoming JSON objects
        let mut json_buffer = String::new();
        while let Some(line) = reader.next_line().await? {
            if line == "{" {
                json_buffer.clear();
            }
            json_buffer.push_str(&line);
            json_buffer.push('\n');
            if line == "}" {
                // Parse JSON
                let parsed_json: IperfResponse =
                    serde_json::from_str::<IperfResponse>(&json_buffer)
                        .expect("Failed to parse JSON");
                self.sender
                    .send(CapEvent::IperfResponse(parsed_json))
                    .expect("Failed to send iperf response");
                json_buffer.clear();
            }
        }
        Ok(())
    }
}


/// Spawns a Tokio task to run a single iperf client test.
///
/// Results are sent back via `sender` as `CapEvent::IperfResponse`.
pub fn dispatch_iperf_client(dest_ip: String, port: u16, duration: u16, sender: CapEventSender) {
    tokio::spawn(async move {
        do_iperf_test(&dest_ip, port, duration, sender).await;
    });
}

/// Executes `iperf3 -c` against `dest_ip:port` for `duration` seconds,
/// reads JSON output, parses into `IperfResponse`, and forwards
/// via `sender`.
pub async fn do_iperf_test(dest_ip: &str, port: u16, duration: u16, sender: CapEventSender) {
    // Build and spawn client process
    let mut cmd = Command::new("iperf3");
    cmd.args([
        "-c",
        dest_ip,
        "-p",
        &port.to_string(),
        "-J",
        "-Z",
        "-t",
        &duration.to_string(),
    ]);


    cmd.stdout(Stdio::piped());
    let mut child = cmd.spawn().expect("Failed to start iperf client");
    let stdout = child.stdout.take().expect("Failed to capture stdout");

    let mut reader = BufReader::new(stdout).lines();

    // Log exit status separately
    tokio::spawn(async move {
        let status = child.wait().await.expect("Failed to wait on child");
        info!("iperf client exited with: {}", status);
    });

    // Buffer JSON and send responses to the parser task
    let mut json_buffer = String::new();
    while let Some(line) = reader.next_line().await.unwrap() {
        if line == "{" {
            json_buffer.clear();
        }

        json_buffer.push_str(&line);
        json_buffer.push('\n');
        if line == "}" {
            // Parse JSON
            let parsed_json =
                serde_json::from_str::<IperfResponse>(&json_buffer).expect("Failed to parse JSON");
            sender
                .send(CapEvent::IperfResponse(parsed_json))
                .expect("Failed to send iperf response");
            json_buffer.clear();
        }
    }
}
