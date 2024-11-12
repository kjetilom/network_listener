use std::error::Error;
use network_listener::listener::{
    capture::PacketCapturer,
    parser::Parser
};
use network_listener::logging::logger;
use log::info;
use network_listener::wireless_listener;

static DO_WIRELESS: bool = false;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    logger::setup_logging()?;
    // let _ = tokio::spawn(network_listener::grafana::client::start_client());

    info!("Starting packet capture");
    let (pcap, receiver, device)
        = PacketCapturer::new()?;



    let cap_h = pcap.start_capture_loop();

    let parser = Parser::new(receiver, device)?;

    if DO_WIRELESS {
        let (mon, mon_recv, _)
        = PacketCapturer::monitor_device(String::from("mon0"))?;

        let _mon_h = mon.start_capture_loop();

        let mon_parser = wireless_listener::parser::Parser::new(mon_recv);
        let _mon_parse_h = tokio::spawn(mon_parser.start());
    }

    let _ = tokio::spawn(parser.start());

    // ! This should be improved
    let _ = cap_h.await?;

    Ok(())
}
