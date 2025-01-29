use std::{
    io::{BufRead, BufReader},
    process::Stdio,
};

use anyhow::Result;
use log::info;

use crate::probe::iperf_json::IperfResponse;


pub struct IperfServer {
    listen_port: u16,
}

impl IperfServer {
    pub fn new(listen_port: u16) -> Result<Self> {
        Ok(IperfServer {
            listen_port,
        })
    }

    fn parse_json(&self, json: &str) {
        // Parse JSON
        let parsed_json = serde_json::from_str::<IperfResponse>(json).expect("Failed to parse JSON");

        // Do sending and stuff here
        dbg!(parsed_json);
    }

    pub async fn start(self) {
        // Run iperf -s -p $port
        let port = self.listen_port;
        info!("Starting iperf server on port {}", port);
        let mut cmd = std::process::Command::new("iperf3")
            .arg("-s")
            .arg("--json")
            .arg("-p")
            .arg(port.to_string())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to start iperf server");

        let stdout = cmd.stdout.take().expect("Failed to capture stdout");
        let reader = BufReader::new(stdout);

        let mut json_buffer = String::new();
        for line_result in reader.lines() {
            match line_result {
                Ok(line) => {
                    if line == "{" {
                        json_buffer.clear();
                    }

                    json_buffer.push_str(&line);
                    json_buffer.push_str("\n");
                    if line == "}" {
                        // Parse JSON
                        info!("Parsing JSON");
                        self.parse_json(&json_buffer);
                        json_buffer.clear();
                    }
                }
                Err(e) => {
                    info!("Error reading line: {:?}", e);
                }
            }
        }
        cmd.kill().expect("Failed to kill iperf server");
    }
}

pub fn do_iperf_test(dest_ip: &str, port: u16) {
    // Run iperf -c $dest_ip -p $port
    let mut cmd = std::process::Command::new("iperf3")
        .arg("-c")
        .arg(dest_ip.to_string())
        .arg("-p")
        .arg(port.to_string())
        .arg("--json")
        .arg("-t")
        .arg("1")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start iperf client");

    let stdout = cmd.stdout.take().expect("Failed to capture stdout");
    let reader = BufReader::new(stdout);

    let mut json_buffer = String::new();

    for line_result in reader.lines() {
        match line_result {
            Ok(line) => {
                if line == "{" {
                    json_buffer.clear();
                }

                json_buffer.push_str(&line);
                json_buffer.push_str("\n");
                if line == "}" {
                    // Parse JSON
                    let parsed_json = serde_json::from_str::<IperfResponse>(&json_buffer).expect("Failed to parse JSON");
                    dbg!(parsed_json);
                    json_buffer.clear();
                }
            }
            Err(e) => {
                info!("Error reading line: {:?}", e);
            }
        }
    }
}
