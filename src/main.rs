use std::error::Error;
use network_listener::listener::{
    capture::PacketCapturer,
    logger,
    parser::Parser
};
use log::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    logger::setup_logging()?;

    info!("Starting packet capture");
    let (pcap, receiver, device)
        = PacketCapturer::new()?;

    pcap.start_capture_loop();

    let parser = Parser::new(receiver, device);
    parser.start().await;



    Ok(())
}
