use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};

use anyhow::Result;
use log::info;
use tokio::process::Command;

use crate::listener::capture::{CapEvent, CapEventSender};
use crate::probe::iperf_json::IperfResponse;

pub enum IperfEvent {
    JSON(String),
}

#[derive(Debug)]
pub struct IperfServer {
    listen_port: u16,
    sender: CapEventSender,
}

impl IperfServer {
    pub fn new(listen_port: u16, sender: CapEventSender) -> Result<Self> {
        Ok(IperfServer {
            listen_port,
            sender,
        })
    }

    pub async fn start(self) -> Result<()> {
        // Run iperf -s -p $port
        let port = self.listen_port;
        info!("Starting iperf server on port {}", port);
        let mut cmd = Command::new("iperf3");

        cmd.args(["-s", "--json", "-p", &port.to_string()]);
        cmd.stdout(Stdio::piped());

        let mut child = cmd.spawn().expect("Failed to start iperf server");

        let stdout = child.stdout.take().expect("Failed to capture stdout");

        let mut reader = BufReader::new(stdout).lines();

        tokio::spawn(async move {
            let status = child.wait().await.expect("Failed to wait on child");
            info!("iperf server exited with: {}", status);
        });

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

pub async fn do_iperf_test(dest_ip: &str, port: u16, duration: u16) {
    // Run iperf -c $dest_ip -p $port
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
    let mut child = cmd.spawn()
        .expect("Failed to start iperf client");
    let stdout = child.stdout.take()
        .expect("Failed to capture stdout");

    let mut reader = BufReader::new(stdout).lines();

    tokio::spawn(async move {
        let status = child.wait().await.expect("Failed to wait on child");
        info!("iperf client exited with: {}", status);
    });

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
            dbg!(parsed_json);
            json_buffer.clear();
        }
    }
}
