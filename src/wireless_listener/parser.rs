use tokio::sync::mpsc::UnboundedReceiver;
use radiotap::{field::Kind, Radiotap};

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

/// Parse a packet into a WirelessPacket
fn parse_packet(packet: &OwnedPacket) -> Option<WirelessPacket> {
    // Parsing logic goes here
    let data = packet.data.as_slice();

    //println!("{:?}", data);

    let (rtap, _) = Radiotap::parse(data).ok()?;
    for field in rtap.header.present.iter() {
        match field {
            Kind::Antenna => {
                println!("{:?}", rtap.antenna);
            },
            Kind::Channel => {
                println!("{:?}", rtap.channel);
            },
            Kind::Rate => {
                println!("{:?}", rtap.rate);
            },
            _ => {}
        }
    }
    None
}