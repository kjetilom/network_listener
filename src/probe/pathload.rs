use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};

use log::info;
use tokio::process::Command;

use crate::*;

pub fn dispatch_server() -> tokio::task::JoinHandle<()> {
    info!("Starting pathload_snd");
    let mut cmd = Command::new("pathload_snd");

    cmd.args(["-q", "-i"]);

    let mut child = cmd.spawn().expect("Failed to start pathload server");

    tokio::spawn(async move {
        let status = child.wait().await.expect("Failed to wait on child");
        info!("pathload server exited with: {}", status);
        return;
    })
}

pub fn dispatch_pathload_client(sender: CapEventSender, ip_addr: String) {
    tokio::spawn(async move {
        do_pathload_test(sender, ip_addr).await;
    });
}

pub async fn do_pathload_test(sender: CapEventSender, ip_addr: String) {
    info!("Starting pathload_rcv");
    let mut cmd = Command::new("pathload_rcv");

    cmd.args(["-q", "-s", &ip_addr, "-N", "/dev/stdout"]);
    cmd.stdout(Stdio::piped());

    let mut child = cmd.spawn().expect("Failed to start pathload_rcv");

    let stdout = child.stdout.take().expect("Failed to capture stdout from pathload_rcv");

    let mut reader = BufReader::new(stdout).lines();

    tokio::spawn(async move {
        let status = child.wait().await.expect("Failed to wait on child");
        info!("pathload client exited with: {}", status);
        return;
    });

    while let Some(line) = reader.next_line().await.unwrap() {
        if line.starts_with("DATE=") {
            sender.send(CapEvent::PathloadResponse(line)).unwrap_or_else(
                |e| {
                    info!("Failed to send pathload response: {}", e)
                }
            );
        }
    }
}