use std::error::Error;
use network_listener::listener::{
    analyzer,
    capture,
    logger,
    parser,
};
use log::{info, debug};

fn main() -> Result<(), Box<dyn Error>> {
    logger::setup_logging()?;

    info!("Starting packet capture");
    let mut pcap = capture::PacketCapturer::new()?;

    let mut analyzer = analyzer::Analyzer::new();

    pcap.capture_loop(|packet| {
        if let Some(parsed_packet) = parser::parse_packet(packet) {
            analyzer.process_packet(&parsed_packet);
        }
    })?;

    Ok(())
}
