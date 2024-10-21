use std::error::Error;
use network_listener::listener::{
    capture::PacketCapturer,
    logger,
    parser::Parser
};
use network_listener::listener::procfs_reader::delay;
use network_listener::listener::procfs_reader::get_socket_info;
use log::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    logger::setup_logging()?;

    info!("Starting packet capture");
    let (pcap, receiver, device)
        = PacketCapturer::new()?;

    pcap.start_capture_loop();

    tokio::spawn(async move {
        delay().await;
    });

    tokio::spawn(async move {
        let _ = get_socket_info().await;
    });

    let parser = Parser::new(receiver, device);
    parser.start().await;



    Ok(())
}
