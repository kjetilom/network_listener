use tokio::sync::mpsc::UnboundedReceiver;
use radiotap::{field, RadiotapIterator};

use crate::listener::capture::OwnedPacket;

#[derive(Debug)]
pub struct WirelessPacket {
    pub source: String,
    pub destination: String,
    pub ssid: Option<String>,
    pub signal_strength: Option<i8>,
}

pub struct Parser {
    packet_stream: UnboundedReceiver<OwnedPacket>,
}


impl Parser {
    pub fn new(packet_stream: UnboundedReceiver<OwnedPacket>) -> Self {
        Parser {
            packet_stream,
        }
    }

    pub async fn start(mut self) {
        while let Some(packet) = self.packet_stream.recv().await {
            if let Some(wireless_packet) = parse_packet(&packet) {
                // Do something with the parsed packet
                println!("{:?}", wireless_packet);
            }
        }
    }
}

fn parse_packet(packet: &OwnedPacket) -> Option<WirelessPacket> {
    // Parsing logic goes here
    let data = packet.data.as_slice();

    // Parse Radiotap header
    for element in RadiotapIterator::from_bytes(&data).unwrap() {
        match element {
            Ok((field::Kind::VHT, data)) => {
                let vht: field::VHT = field::from_bytes(data).unwrap();
                println!("{:?}", vht);
            },
            _ => {}
        }
    }
    None
}