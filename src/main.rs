extern crate pnet;

use pnet::datalink::{self, NetworkInterface, Config};
use pnet::packet::ethernet::EthernetPacket;
use pnet::datalink::Channel::Ethernet;

fn handle_packet(packet: &EthernetPacket) {
    println!("Received packet: {:?}", packet);
}

fn find_interface() -> Option<NetworkInterface> {
    let interfaces = datalink::interfaces();
    let iface = interfaces.into_iter()
        .find(|iface| iface.is_up() && !iface.is_loopback() && iface.is_broadcast());
    println!("Listening on interface: {:?}", iface);
    iface
}

fn main() {
    let interface = find_interface().expect("No suitable interface found");

    let config = Config::default();
    let channel = datalink::channel(&interface, config)
                             .expect("Failed to create datalink channel");

    match channel {
        Ethernet(_, mut rx) => {
            loop {
                match rx.next() {
                    Ok(packet) => {
                        let packet = EthernetPacket::new(packet).unwrap();
                        handle_packet(&packet);
                    },
                    Err(e) => {
                        eprintln!("An error occurred while reading: {}", e);
                    }
                }
            }
        },
        _ => eprintln!("Unsupported channel type"),
    }
}
