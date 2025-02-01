/**
 * Module for managing iperf probes.
 *
 * This module provides an implementation for both iperf server and client functionality.
 *
 * IperfEvent:
 *   - Represents events originating from the iperf process output.
 *   - Currently supports events with JSON-formatted data.
 *
 * IperfServer:
 *   - A structure representing an iperf server that listens on a specified port.
 *   - It spawns an iperf3 process to run as a server, captures its JSON output, and dispatches
 *     the parsed events via a provided event sender channel.
 *
 * Methods on IperfServer:
 *   - new:
 *       * Initializes the server with a given listening port and event sender.
 *   - dispatch_server:
 *       * Spawns the server operation asynchronously.
 *   - start:
 *       * Launches the iperf server process and processes its stdout line-by-line, constructing JSON
 *         messages and sending parsed responses as events.
 *
 * do_iperf_test:
 *   - Executes an iperf client test to a specified destination IP and port.
 *   - Runs iperf3 in client mode with options for JSON output, capturing and processing the output
 *     to provide test results.
 *
 * Common Functionality:
 *   - Both client and server functions use asynchronous process management via Tokio's facilities.
 *   - Process output is streamed using asynchronous buffered readers, and logging is performed
 *     to track process lifecycle and errors.
 *
 * Error Handling:
 *   - Uses the anyhow crate to simplify error propagation and reporting in asynchronous contexts.
 */
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};

use anyhow::Result;
use log::info;
use tokio::process::Command;

use crate::*;
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

    pub fn dispatch_server(self) -> tokio::task::JoinHandle<Result<()>> {
        tokio::spawn(async move { self.start().await })
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
