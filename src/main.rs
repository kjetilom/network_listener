use std::error::Error;
use network_listener::listener::{
    capture::PacketCapturer,
    parser::Parser
};
use network_listener::logging::logger;
use log::info;
use network_listener::wireless_listener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    logger::setup_logging()?;

    info!("Starting packet capture");
    let (pcap, receiver, device)
        = PacketCapturer::new()?;



    pcap.start_capture_loop();

    let parser = Parser::new(receiver, device);
    parser.start().await;

    // let (mon, mon_recv, mon_device)
    // = PacketCapturer::monitor_device(String::from("mon0"))?;

    // mon.start_capture_loop();

    // let mon_parser = wireless_listener::parser::Parser::new(mon_recv);
    // tokio::spawn(mon_parser.start());

    Ok(())
}
