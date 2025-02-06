use futures::StreamExt;
use prost::Message;
use std::env;
use std::error::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

// Adjust the module path to match your generated protobuf code.
use network_listener::proto_bw::HelloMessage;

async fn handle_connection(socket: TcpStream) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Wrap the socket with a length-delimited codec for framing.
    let mut framed = Framed::new(socket, LengthDelimitedCodec::new());

    // Wait for a complete frame (a complete Protobuf message)
    if let Some(frame) = framed.next().await {
        let bytes = frame?;
        let msg = HelloMessage::decode(bytes)?;
        println!("Received message: {}", msg.message);
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
